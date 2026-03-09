use crate::error::FontsourceError;
use crate::types::FontStyle;
use read_fonts::types::GlyphId;
use read_fonts::{FontRead, FontRef, TableProvider};
use skrifa::MetadataProvider;
use std::collections::BTreeMap;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gpos::PositionLookup;
use write_fonts::tables::gsub::SubstitutionLookup;
use write_fonts::tables::layout::CoverageTable;
use write_fonts::types::GlyphId16;
use write_fonts::FontBuilder;

/// Merge multiple subset TTF byte slices into a single TTF.
///
/// Uses cmap-based GID remapping: each subset may have its own compact GID
/// namespace, so we assign new sequential GIDs based on cmap entries and
/// remap composite glyph references accordingly.
pub(crate) fn merge_subsets(
    font_id: &str,
    weight: u16,
    style: FontStyle,
    subsets: &[&[u8]],
) -> Result<Vec<u8>, FontsourceError> {
    let make_err = |message: String| FontsourceError::FontMerge {
        font_id: font_id.to_string(),
        weight,
        style: style.as_str().to_string(),
        message,
    };

    if subsets.len() == 1 {
        return Ok(subsets[0].to_vec());
    }

    // Parse all subset fonts
    let fonts: Vec<FontRef> = subsets
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            FontRef::new(bytes).map_err(|e| make_err(format!("Failed to parse subset {i}: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Verify TrueType outlines (glyf table must be present)
    if fonts[0].glyf().is_err() {
        return Err(make_err(
            "Font has no glyf table (CFF outlines not supported for merging)".to_string(),
        ));
    }

    // === Step 1: Collect cmap entries from all subsets ===
    // codepoint -> (subset_idx, old_gid)
    let mut codepoint_to_source: BTreeMap<u32, (usize, u32)> = BTreeMap::new();
    for (i, bytes) in subsets.iter().enumerate() {
        let skrifa_font = skrifa::FontRef::new(bytes)
            .map_err(|e| make_err(format!("Failed to parse subset {i} with skrifa: {e}")))?;
        let charmap = skrifa_font.charmap();
        for (codepoint, glyph_id) in charmap.mappings() {
            // First mapping wins (base/default subset is first)
            codepoint_to_source
                .entry(codepoint)
                .or_insert((i, glyph_id.to_u32()));
        }
    }

    // === Step 2: Assign new GIDs ===
    // Key: (subset_idx, old_gid) -> new_gid
    let mut glyph_map: BTreeMap<(usize, u32), u32> = BTreeMap::new();
    let mut glyph_order: Vec<(usize, u32)> = Vec::new();

    // GID 0 = .notdef from base font
    glyph_map.insert((0, 0), 0);
    glyph_order.push((0, 0));
    let mut next_gid = 1u32;

    for &(subset_idx, old_gid) in codepoint_to_source.values() {
        let key = (subset_idx, old_gid);
        if let std::collections::btree_map::Entry::Vacant(e) = glyph_map.entry(key) {
            e.insert(next_gid);
            glyph_order.push(key);
            next_gid += 1;
        }
    }

    // === Step 3: Add composite glyph components (recursive) ===
    let mut scan_idx = 0;
    while scan_idx < glyph_order.len() {
        let (subset_idx, old_gid) = glyph_order[scan_idx];
        if let Some(raw) = raw_glyph_bytes(&fonts[subset_idx], old_gid) {
            if is_composite_glyph(raw) {
                for comp_gid in extract_composite_component_gids(raw) {
                    let key = (subset_idx, comp_gid);
                    if let std::collections::btree_map::Entry::Vacant(e) = glyph_map.entry(key) {
                        e.insert(next_gid);
                        glyph_order.push(key);
                        next_gid += 1;
                    }
                }
            }
        }
        scan_idx += 1;
    }

    // === Step 3b: Add GSUB/GPOS closure glyphs from base font ===
    // GSUB lookups may reference output GIDs (ligatures, alternates, final forms)
    // that aren't in the cmap. We need to include these glyphs so the remapped
    // GSUB/GPOS tables reference valid GIDs in the merged font.
    collect_gsub_closure_glyphs(&fonts[0], &mut glyph_map, &mut glyph_order, &mut next_gid);

    let total_glyphs = next_gid;
    if total_glyphs > u16::MAX as u32 {
        return Err(make_err(format!(
            "Merged glyph count ({total_glyphs}) exceeds maximum of {}",
            u16::MAX
        )));
    }

    // === Step 4: Build glyf and loca tables from raw bytes ===
    let mut glyf_buf = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(total_glyphs as usize + 1);

    for &(subset_idx, old_gid) in &glyph_order {
        offsets.push(glyf_buf.len() as u32);
        if let Some(raw) = raw_glyph_bytes(&fonts[subset_idx], old_gid) {
            if is_composite_glyph(raw) {
                let remapped = remap_composite_gids(raw, subset_idx, &glyph_map);
                glyf_buf.extend_from_slice(&remapped);
            } else {
                glyf_buf.extend_from_slice(raw);
            }
            // Pad to 2-byte alignment for short loca format compatibility
            if glyf_buf.len() % 2 != 0 {
                glyf_buf.push(0);
            }
        }
    }
    offsets.push(glyf_buf.len() as u32);

    // Choose loca format: short if all offsets fit in u16*2, long otherwise
    let loca_format: i16 = if glyf_buf.len() > 0x1FFFE { 1 } else { 0 };
    let loca_buf = build_loca_bytes(&offsets, loca_format);

    // === Step 5: Build cmap ===
    let mut new_cmap: BTreeMap<u32, GlyphId> = BTreeMap::new();
    for (&cp, &(subset_idx, old_gid)) in &codepoint_to_source {
        if let Some(&new_gid) = glyph_map.get(&(subset_idx, old_gid)) {
            new_cmap.insert(cp, GlyphId::new(new_gid));
        }
    }
    let cmap_bytes = build_cmap_bytes(&new_cmap);

    // === Step 6: Build hmtx from remapped glyph order ===
    let hmtx_bytes = build_hmtx_remapped(&fonts, &glyph_order, &make_err)?;

    // === Step 7: Update head, maxp, hhea tables ===
    let base = &fonts[0];
    let mut head: write_fonts::tables::head::Head = base
        .head()
        .map_err(|e| make_err(format!("Failed to read head: {e}")))?
        .to_owned_table();
    head.index_to_loc_format = loca_format;

    let mut maxp: write_fonts::tables::maxp::Maxp = base
        .maxp()
        .map_err(|e| make_err(format!("Failed to read maxp: {e}")))?
        .to_owned_table();
    maxp.num_glyphs = total_glyphs as u16;

    // Copy raw hhea and patch numberOfHMetrics (last 2 bytes)
    let hhea_data = base
        .data_for_tag(read_fonts::types::Tag::new(b"hhea"))
        .ok_or_else(|| make_err("Missing hhea table".to_string()))?;
    let mut hhea_bytes = hhea_data.as_bytes().to_vec();
    let len = hhea_bytes.len();
    if len >= 2 {
        hhea_bytes[len - 2..].copy_from_slice(&(total_glyphs as u16).to_be_bytes());
    }

    // === Step 8: Remap GSUB/GPOS/GDEF tables ===
    // Build old_gid→new_gid mapping for base font (subset 0) since GSUB/GPOS/GDEF
    // come from the base font only
    let base_gid_remap: BTreeMap<u32, u32> = glyph_map
        .iter()
        .filter(|&(&(subset_idx, _), _)| subset_idx == 0)
        .map(|(&(_, old_gid), &new_gid)| (old_gid, new_gid))
        .collect();

    // === Step 9: Assemble with FontBuilder ===
    let mut builder = FontBuilder::new();
    builder
        .add_table(&head)
        .map_err(|e| make_err(format!("Failed to add head table: {e}")))?;
    builder.add_raw(read_fonts::types::Tag::new(b"hhea"), hhea_bytes);
    builder
        .add_table(&maxp)
        .map_err(|e| make_err(format!("Failed to add maxp table: {e}")))?;

    builder.add_raw(read_fonts::types::Tag::new(b"glyf"), glyf_buf);
    builder.add_raw(read_fonts::types::Tag::new(b"loca"), loca_buf);
    builder.add_raw(read_fonts::types::Tag::new(b"cmap"), cmap_bytes);
    builder.add_raw(read_fonts::types::Tag::new(b"hmtx"), hmtx_bytes);

    // Remap and add GSUB/GPOS/GDEF before copy_missing_tables so originals
    // are not copied (copy_missing_tables skips tags already present).
    // If remapping fails but the table exists, add an empty entry to prevent
    // copy_missing_tables from copying the stale original (which has pre-remap GIDs).
    for (tag_bytes, remap_fn) in [
        (
            b"GSUB",
            remap_gsub_table as fn(&FontRef, &BTreeMap<u32, u32>) -> Option<Vec<u8>>,
        ),
        (
            b"GPOS",
            remap_gpos_table as fn(&FontRef, &BTreeMap<u32, u32>) -> Option<Vec<u8>>,
        ),
        (
            b"GDEF",
            remap_gdef_table as fn(&FontRef, &BTreeMap<u32, u32>) -> Option<Vec<u8>>,
        ),
    ] {
        let tag = read_fonts::types::Tag::new(tag_bytes);
        if let Some(remapped) = remap_fn(base, &base_gid_remap) {
            builder.add_raw(tag, remapped);
        } else if base.data_for_tag(tag).is_some() {
            // Table exists but remap failed — drop it rather than copying stale GIDs
            builder.add_raw(tag, vec![]);
        }
    }

    // Build a format 3.0 post table (no glyph names) to replace the base font's
    // post table, whose numGlyphs/glyphNameIndex would be stale after GID remapping.
    let post_v3 = build_post_v3(base, &make_err)?;
    builder.add_raw(read_fonts::types::Tag::new(b"post"), post_v3);

    // Copy remaining tables from base font (name, OS/2, etc.)
    // post is already present so copy_missing_tables will skip the original.
    let base_for_copy = FontRef::new(subsets[0])
        .map_err(|e| make_err(format!("Failed to re-parse base font: {e}")))?;
    builder.copy_missing_tables(base_for_copy);

    Ok(builder.build())
}

/// Get the raw bytes for a glyph from a font by reading loca offsets into glyf data.
fn raw_glyph_bytes<'a>(font: &FontRef<'a>, gid: u32) -> Option<&'a [u8]> {
    let head = font.head().ok()?;
    let loca_data = font
        .data_for_tag(read_fonts::types::Tag::new(b"loca"))?
        .as_bytes();
    let glyf_data = font
        .data_for_tag(read_fonts::types::Tag::new(b"glyf"))?
        .as_bytes();
    let is_long = head.index_to_loc_format() == 1;

    let (start, end) = if is_long {
        let idx = gid as usize * 4;
        if idx + 8 > loca_data.len() {
            return None;
        }
        let s = u32::from_be_bytes(loca_data[idx..idx + 4].try_into().ok()?) as usize;
        let e = u32::from_be_bytes(loca_data[idx + 4..idx + 8].try_into().ok()?) as usize;
        (s, e)
    } else {
        let idx = gid as usize * 2;
        if idx + 4 > loca_data.len() {
            return None;
        }
        let s = u16::from_be_bytes(loca_data[idx..idx + 2].try_into().ok()?) as usize * 2;
        let e = u16::from_be_bytes(loca_data[idx + 2..idx + 4].try_into().ok()?) as usize * 2;
        (s, e)
    };

    if start >= end || end > glyf_data.len() {
        return None; // empty glyph
    }
    Some(&glyf_data[start..end])
}

/// Check if raw glyph bytes represent a composite glyph (numberOfContours < 0).
fn is_composite_glyph(bytes: &[u8]) -> bool {
    if bytes.len() < 2 {
        return false;
    }
    let num_contours = i16::from_be_bytes([bytes[0], bytes[1]]);
    num_contours < 0
}

// TrueType composite glyph flags
const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;

/// Walk composite glyph binary data and return (byte_offset_of_glyphIndex, old_gid) pairs.
fn walk_composite_components(bytes: &[u8]) -> Vec<(usize, u32)> {
    let mut result = Vec::new();
    let mut offset = 10; // Skip glyph header: numberOfContours(i16) + xMin/yMin/xMax/yMax (4*i16)

    loop {
        if offset + 4 > bytes.len() {
            break;
        }
        let flags = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        let glyph_index = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]);
        result.push((offset + 2, glyph_index as u32));

        offset += 4; // past flags + glyphIndex

        // Skip arguments
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            offset += 4; // two i16/u16
        } else {
            offset += 2; // two i8/u8
        }

        // Skip transform
        if flags & WE_HAVE_A_SCALE != 0 {
            offset += 2; // one F2Dot14
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            offset += 4; // two F2Dot14
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            offset += 8; // four F2Dot14
        }

        if flags & MORE_COMPONENTS == 0 {
            break;
        }
    }

    result
}

/// Extract component glyph IDs from composite glyph bytes.
fn extract_composite_component_gids(bytes: &[u8]) -> Vec<u32> {
    walk_composite_components(bytes)
        .into_iter()
        .map(|(_, gid)| gid)
        .collect()
}

/// Clone composite glyph bytes with component GIDs remapped to new values.
fn remap_composite_gids(
    bytes: &[u8],
    subset_idx: usize,
    glyph_map: &BTreeMap<(usize, u32), u32>,
) -> Vec<u8> {
    let mut result = bytes.to_vec();
    for (glyph_index_offset, old_gid) in walk_composite_components(bytes) {
        if let Some(&new_gid) = glyph_map.get(&(subset_idx, old_gid)) {
            result[glyph_index_offset..glyph_index_offset + 2]
                .copy_from_slice(&(new_gid as u16).to_be_bytes());
        }
    }
    result
}

/// Build a cmap table using Format 12 (segmented coverage).
fn build_cmap_bytes(mappings: &BTreeMap<u32, GlyphId>) -> Vec<u8> {
    let mut groups: Vec<(u32, u32, u32)> = Vec::new(); // (start_char, end_char, start_glyph)

    for (&codepoint, &glyph_id) in mappings {
        let gid = glyph_id.to_u32();
        if let Some(last) = groups.last_mut() {
            if codepoint == last.1 + 1 && gid == last.2 + (codepoint - last.0) {
                last.1 = codepoint;
                continue;
            }
        }
        groups.push((codepoint, codepoint, gid));
    }

    let num_groups = groups.len() as u32;
    let subtable_length = 16 + 12 * num_groups;
    let total_size = 4 + 8 + subtable_length as usize;
    let mut buf = Vec::with_capacity(total_size);

    // cmap header
    buf.extend_from_slice(&0u16.to_be_bytes()); // version
    buf.extend_from_slice(&1u16.to_be_bytes()); // numTables

    // Encoding record: platform 3 (Windows), encoding 10 (Unicode full)
    buf.extend_from_slice(&3u16.to_be_bytes()); // platformID
    buf.extend_from_slice(&10u16.to_be_bytes()); // encodingID
    buf.extend_from_slice(&12u32.to_be_bytes()); // offset to subtable

    // Format 12 subtable
    buf.extend_from_slice(&12u16.to_be_bytes()); // format
    buf.extend_from_slice(&0u16.to_be_bytes()); // reserved
    buf.extend_from_slice(&subtable_length.to_be_bytes()); // length
    buf.extend_from_slice(&0u32.to_be_bytes()); // language
    buf.extend_from_slice(&num_groups.to_be_bytes()); // numGroups

    for (start_char, end_char, start_glyph) in &groups {
        buf.extend_from_slice(&start_char.to_be_bytes());
        buf.extend_from_slice(&end_char.to_be_bytes());
        buf.extend_from_slice(&start_glyph.to_be_bytes());
    }

    buf
}

/// Build hmtx table for the remapped glyph order.
/// Each entry in glyph_order is (subset_idx, old_gid).
fn build_hmtx_remapped(
    fonts: &[FontRef],
    glyph_order: &[(usize, u32)],
    make_err: &impl Fn(String) -> FontsourceError,
) -> Result<Vec<u8>, FontsourceError> {
    let mut buf = Vec::with_capacity(glyph_order.len() * 4);

    for &(subset_idx, old_gid) in glyph_order {
        let font = &fonts[subset_idx];
        let hmtx = font
            .hmtx()
            .map_err(|e| make_err(format!("Failed to read hmtx: {e}")))?;
        let hhea = font
            .hhea()
            .map_err(|e| make_err(format!("Failed to read hhea: {e}")))?;
        let num_long = hhea.number_of_h_metrics();
        let gid_u16 = old_gid as u16;

        if gid_u16 < num_long {
            if let Some(metrics) = hmtx.h_metrics().get(gid_u16 as usize) {
                buf.extend_from_slice(&metrics.advance().to_be_bytes());
                buf.extend_from_slice(&metrics.side_bearing().to_be_bytes());
                continue;
            }
        } else {
            // Glyph uses last long metric's advance + left_side_bearings array
            let last_advance = if num_long > 0 {
                hmtx.h_metrics()
                    .get(num_long as usize - 1)
                    .map(|m| m.advance())
                    .unwrap_or(0)
            } else {
                0
            };
            let lsb_idx = (gid_u16 - num_long) as usize;
            let lsb = hmtx
                .left_side_bearings()
                .get(lsb_idx)
                .map(|b| b.get())
                .unwrap_or(0);
            buf.extend_from_slice(&last_advance.to_be_bytes());
            buf.extend_from_slice(&lsb.to_be_bytes());
            continue;
        }

        // Fallback: zero metrics
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&0i16.to_be_bytes());
    }

    Ok(buf)
}

/// Build loca table bytes from glyph offsets.
fn build_loca_bytes(offsets: &[u32], format: i16) -> Vec<u8> {
    if format == 0 {
        // Short format: each entry is u16 = offset / 2
        let mut buf = Vec::with_capacity(offsets.len() * 2);
        for &off in offsets {
            buf.extend_from_slice(&((off / 2) as u16).to_be_bytes());
        }
        buf
    } else {
        // Long format: each entry is u32
        let mut buf = Vec::with_capacity(offsets.len() * 4);
        for &off in offsets {
            buf.extend_from_slice(&off.to_be_bytes());
        }
        buf
    }
}

/// Build a 32-byte post table with format version 3.0 (no glyph names).
///
/// Copies italic angle, underline metrics, and fixed-pitch flag from the base font's
/// existing post table header, but drops per-glyph name data that would be stale after
/// GID remapping.
fn build_post_v3(
    base: &FontRef,
    make_err: &dyn Fn(String) -> FontsourceError,
) -> Result<Vec<u8>, FontsourceError> {
    let raw = base
        .data_for_tag(read_fonts::types::Tag::new(b"post"))
        .ok_or_else(|| make_err("Missing post table".to_string()))?;
    let src = raw.as_bytes();
    if src.len() < 32 {
        return Err(make_err(format!(
            "post table too short: {} bytes",
            src.len()
        )));
    }
    let mut buf = vec![0u8; 32];
    // Version = 3.0 (0x00030000)
    buf[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
    // Copy italicAngle(4..8), underlinePosition(8..10), underlineThickness(10..12),
    // isFixedPitch(12..16) from original header
    buf[4..16].copy_from_slice(&src[4..16]);
    // minMemType42, maxMemType42, minMemType1, maxMemType1 left as zero
    Ok(buf)
}

/// Remap a GlyphId16 using the gid_map, falling back to .notdef (0) for unmapped GIDs.
fn remap_gid16(gid: GlyphId16, gid_map: &BTreeMap<u32, u32>) -> GlyphId16 {
    let old = gid.to_u32();
    let new = gid_map.get(&old).copied().unwrap_or(0);
    GlyphId16::new(new as u16)
}

/// Remap a CoverageTable, returning the remapped coverage and a permutation vector.
///
/// The permutation vector maps old coverage indices to new coverage indices.
/// This is needed because Coverage entries must be sorted by GID, and remapping
/// may change the sort order. Any parallel arrays indexed by coverage index
/// must be permuted accordingly.
fn remap_coverage(
    coverage: &CoverageTable,
    gid_map: &BTreeMap<u32, u32>,
) -> (CoverageTable, Vec<usize>) {
    // Expand coverage to (old_index, old_gid) pairs
    let old_gids: Vec<(usize, u32)> = match coverage {
        CoverageTable::Format1(f1) => f1
            .glyph_array
            .iter()
            .enumerate()
            .map(|(i, g)| (i, g.to_u32()))
            .collect(),
        CoverageTable::Format2(f2) => {
            let mut entries = Vec::new();
            let mut idx = 0usize;
            for range in &f2.range_records {
                let start = range.start_glyph_id.to_u32();
                let end = range.end_glyph_id.to_u32();
                for gid in start..=end {
                    entries.push((idx, gid));
                    idx += 1;
                }
            }
            entries
        }
    };

    // Remap GIDs
    let mut remapped: Vec<(usize, u32)> = old_gids
        .iter()
        .map(|&(old_idx, old_gid)| {
            let new_gid = gid_map.get(&old_gid).copied().unwrap_or(0);
            (old_idx, new_gid)
        })
        .collect();

    // Sort by new GID (Coverage must be sorted)
    remapped.sort_by_key(|&(_, new_gid)| new_gid);

    // Build permutation: permutation[new_index] = old_index
    let permutation: Vec<usize> = remapped.iter().map(|&(old_idx, _)| old_idx).collect();

    // Build new Coverage as Format 1 (sorted glyph array)
    let glyph_array: Vec<GlyphId16> = remapped
        .iter()
        .map(|&(_, gid)| GlyphId16::new(gid as u16))
        .collect();

    (CoverageTable::format_1(glyph_array), permutation)
}

/// Apply a permutation to a vector: result[i] = source[permutation[i]]
fn permute_vec<T: Clone>(source: &[T], permutation: &[usize]) -> Vec<T> {
    permutation.iter().map(|&i| source[i].clone()).collect()
}

/// Remap a ClassDef table.
fn remap_class_def(
    class_def: &write_fonts::tables::layout::ClassDef,
    gid_map: &BTreeMap<u32, u32>,
) -> write_fonts::tables::layout::ClassDef {
    use write_fonts::tables::layout::{ClassDefFormat2, ClassRangeRecord};
    match class_def {
        write_fonts::tables::layout::ClassDef::Format1(f1) => {
            // Expand to (gid, class) pairs, remap, rebuild as Format 2
            let start = f1.start_glyph_id.to_u32();
            let mut pairs: Vec<(u32, u16)> = f1
                .class_value_array
                .iter()
                .enumerate()
                .map(|(i, &class)| {
                    let old_gid = start + i as u32;
                    let new_gid = gid_map.get(&old_gid).copied().unwrap_or(0);
                    (new_gid, class)
                })
                .filter(|&(_, class)| class != 0) // Class 0 is default, no need to store
                .collect();
            pairs.sort_by_key(|&(gid, _)| gid);

            // Build range records from sorted pairs
            let mut ranges: Vec<ClassRangeRecord> = Vec::new();
            for &(gid, class) in &pairs {
                if let Some(last) = ranges.last_mut() {
                    if gid == last.end_glyph_id.to_u32() + 1 && class == last.class {
                        last.end_glyph_id = GlyphId16::new(gid as u16);
                        continue;
                    }
                }
                ranges.push(ClassRangeRecord {
                    start_glyph_id: GlyphId16::new(gid as u16),
                    end_glyph_id: GlyphId16::new(gid as u16),
                    class,
                });
            }
            write_fonts::tables::layout::ClassDef::Format2(ClassDefFormat2::new(ranges))
        }
        write_fonts::tables::layout::ClassDef::Format2(f2) => {
            // Expand ranges to (gid, class) pairs, remap, rebuild
            let mut pairs: Vec<(u32, u16)> = Vec::new();
            for range in &f2.class_range_records {
                let start = range.start_glyph_id.to_u32();
                let end = range.end_glyph_id.to_u32();
                for gid in start..=end {
                    let new_gid = gid_map.get(&gid).copied().unwrap_or(0);
                    if range.class != 0 {
                        pairs.push((new_gid, range.class));
                    }
                }
            }
            pairs.sort_by_key(|&(gid, _)| gid);

            let mut ranges: Vec<write_fonts::tables::layout::ClassRangeRecord> = Vec::new();
            for &(gid, class) in &pairs {
                if let Some(last) = ranges.last_mut() {
                    if gid == last.end_glyph_id.to_u32() + 1 && class == last.class {
                        last.end_glyph_id = GlyphId16::new(gid as u16);
                        continue;
                    }
                }
                ranges.push(write_fonts::tables::layout::ClassRangeRecord {
                    start_glyph_id: GlyphId16::new(gid as u16),
                    end_glyph_id: GlyphId16::new(gid as u16),
                    class,
                });
            }
            write_fonts::tables::layout::ClassDef::Format2(
                write_fonts::tables::layout::ClassDefFormat2::new(ranges),
            )
        }
    }
}

/// Collect output GIDs from GSUB lookups in the base font to ensure they're
/// included in the merged font's glyph set.
fn collect_gsub_closure_glyphs(
    base_font: &FontRef,
    glyph_map: &mut BTreeMap<(usize, u32), u32>,
    glyph_order: &mut Vec<(usize, u32)>,
    next_gid: &mut u32,
) {
    let gsub_data = match base_font.data_for_tag(read_fonts::types::Tag::new(b"GSUB")) {
        Some(data) => data,
        None => return,
    };

    let gsub: write_fonts::tables::gsub::Gsub =
        match read_fonts::tables::gsub::Gsub::read(gsub_data) {
            Ok(parsed) => parsed.to_owned_table(),
            Err(_) => return,
        };

    let mut closure_gids: Vec<u32> = Vec::new();

    for lookup in &gsub.lookup_list.lookups {
        collect_lookup_output_gids(lookup, &mut closure_gids);
    }

    // Add closure GIDs to glyph_map (as base font glyphs, subset_idx=0)
    for gid in closure_gids {
        let key = (0usize, gid);
        if let std::collections::btree_map::Entry::Vacant(e) = glyph_map.entry(key) {
            e.insert(*next_gid);
            glyph_order.push(key);
            *next_gid += 1;
        }
    }
}

/// Collect output GIDs from a single GSUB lookup.
fn collect_lookup_output_gids(lookup: &SubstitutionLookup, output: &mut Vec<u32>) {
    match lookup {
        SubstitutionLookup::Single(l) => {
            for subtable in &l.subtables {
                match subtable.as_ref() {
                    write_fonts::tables::gsub::SingleSubst::Format1(f1) => {
                        // Output = input + delta. Collect all possible outputs.
                        let delta = f1.delta_glyph_id as i32;
                        for gid in coverage_gids(&f1.coverage) {
                            let out = (gid as i32 + delta) as u32;
                            output.push(out);
                        }
                    }
                    write_fonts::tables::gsub::SingleSubst::Format2(f2) => {
                        for gid in &f2.substitute_glyph_ids {
                            output.push(gid.to_u32());
                        }
                    }
                }
            }
        }
        SubstitutionLookup::Multiple(l) => {
            for subtable in &l.subtables {
                for seq in &subtable.sequences {
                    for gid in &seq.substitute_glyph_ids {
                        output.push(gid.to_u32());
                    }
                }
            }
        }
        SubstitutionLookup::Alternate(l) => {
            for subtable in &l.subtables {
                for alt_set in &subtable.alternate_sets {
                    for gid in &alt_set.alternate_glyph_ids {
                        output.push(gid.to_u32());
                    }
                }
            }
        }
        SubstitutionLookup::Ligature(l) => {
            for subtable in &l.subtables {
                for lig_set in &subtable.ligature_sets {
                    for lig in &lig_set.ligatures {
                        output.push(lig.ligature_glyph.to_u32());
                    }
                }
            }
        }
        SubstitutionLookup::Reverse(l) => {
            for subtable in &l.subtables {
                for gid in &subtable.substitute_glyph_ids {
                    output.push(gid.to_u32());
                }
            }
        }
        SubstitutionLookup::Extension(l) => {
            for subtable in &l.subtables {
                match subtable.as_ref() {
                    write_fonts::tables::gsub::ExtensionSubtable::Single(s) => {
                        match &*s.extension {
                            write_fonts::tables::gsub::SingleSubst::Format1(f1) => {
                                let delta = f1.delta_glyph_id as i32;
                                for gid in coverage_gids(&f1.coverage) {
                                    output.push((gid as i32 + delta) as u32);
                                }
                            }
                            write_fonts::tables::gsub::SingleSubst::Format2(f2) => {
                                for gid in &f2.substitute_glyph_ids {
                                    output.push(gid.to_u32());
                                }
                            }
                        }
                    }
                    write_fonts::tables::gsub::ExtensionSubtable::Multiple(s) => {
                        for seq in &s.extension.sequences {
                            for gid in &seq.substitute_glyph_ids {
                                output.push(gid.to_u32());
                            }
                        }
                    }
                    write_fonts::tables::gsub::ExtensionSubtable::Alternate(s) => {
                        for alt_set in &s.extension.alternate_sets {
                            for gid in &alt_set.alternate_glyph_ids {
                                output.push(gid.to_u32());
                            }
                        }
                    }
                    write_fonts::tables::gsub::ExtensionSubtable::Ligature(s) => {
                        for lig_set in &s.extension.ligature_sets {
                            for lig in &lig_set.ligatures {
                                output.push(lig.ligature_glyph.to_u32());
                            }
                        }
                    }
                    write_fonts::tables::gsub::ExtensionSubtable::Reverse(s) => {
                        for gid in &s.extension.substitute_glyph_ids {
                            output.push(gid.to_u32());
                        }
                    }
                    _ => {} // Context/ChainContext don't directly produce output GIDs
                }
            }
        }
        // Context/ChainContext lookups reference other lookups, not direct GID outputs
        SubstitutionLookup::Contextual(_) | SubstitutionLookup::ChainContextual(_) => {}
    }
}

/// Extract all GIDs from a CoverageTable.
fn coverage_gids(coverage: &CoverageTable) -> Vec<u32> {
    match coverage {
        CoverageTable::Format1(f1) => f1.glyph_array.iter().map(|g| g.to_u32()).collect(),
        CoverageTable::Format2(f2) => {
            let mut gids = Vec::new();
            for range in &f2.range_records {
                for gid in range.start_glyph_id.to_u32()..=range.end_glyph_id.to_u32() {
                    gids.push(gid);
                }
            }
            gids
        }
    }
}

/// Remap the GSUB table. Returns None if the font has no GSUB table.
fn remap_gsub_table(base_font: &FontRef, gid_map: &BTreeMap<u32, u32>) -> Option<Vec<u8>> {
    let gsub_data = base_font.data_for_tag(read_fonts::types::Tag::new(b"GSUB"))?;
    let parsed = read_fonts::tables::gsub::Gsub::read(gsub_data).ok()?;
    let mut gsub: write_fonts::tables::gsub::Gsub = parsed.to_owned_table();

    for lookup in gsub.lookup_list.lookups.iter_mut() {
        remap_substitution_lookup(lookup, gid_map);
    }

    write_fonts::dump_table(&gsub).ok()
}

/// Remap all GID references in a GSUB substitution lookup.
fn remap_substitution_lookup(lookup: &mut SubstitutionLookup, gid_map: &BTreeMap<u32, u32>) {
    match lookup {
        SubstitutionLookup::Single(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_single_subst(subtable.as_mut(), gid_map);
            }
        }
        SubstitutionLookup::Multiple(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_cov, perm) = remap_coverage(&subtable.coverage, gid_map);
                *subtable.coverage = new_cov;
                // Remap GIDs in sequence tables and permute
                let old_sequences: Vec<_> = subtable.sequences.clone();
                subtable.sequences = permute_vec(&old_sequences, &perm);
                for seq in subtable.sequences.iter_mut() {
                    for gid in seq.substitute_glyph_ids.iter_mut() {
                        *gid = remap_gid16(*gid, gid_map);
                    }
                }
            }
        }
        SubstitutionLookup::Alternate(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_cov, perm) = remap_coverage(&subtable.coverage, gid_map);
                *subtable.coverage = new_cov;
                let old_sets: Vec<_> = subtable.alternate_sets.clone();
                subtable.alternate_sets = permute_vec(&old_sets, &perm);
                for alt_set in subtable.alternate_sets.iter_mut() {
                    for gid in alt_set.alternate_glyph_ids.iter_mut() {
                        *gid = remap_gid16(*gid, gid_map);
                    }
                }
            }
        }
        SubstitutionLookup::Ligature(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_cov, perm) = remap_coverage(&subtable.coverage, gid_map);
                *subtable.coverage = new_cov;
                let old_sets: Vec<_> = subtable.ligature_sets.clone();
                subtable.ligature_sets = permute_vec(&old_sets, &perm);
                for lig_set in subtable.ligature_sets.iter_mut() {
                    for lig in lig_set.ligatures.iter_mut() {
                        lig.ligature_glyph = remap_gid16(lig.ligature_glyph, gid_map);
                        for gid in lig.component_glyph_ids.iter_mut() {
                            *gid = remap_gid16(*gid, gid_map);
                        }
                    }
                }
            }
        }
        SubstitutionLookup::Contextual(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_sequence_context(subtable.as_mut(), gid_map);
            }
        }
        SubstitutionLookup::ChainContextual(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_chain_context(subtable.as_mut(), gid_map);
            }
        }
        SubstitutionLookup::Extension(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_gsub_extension(subtable.as_mut(), gid_map);
            }
        }
        SubstitutionLookup::Reverse(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_cov, perm) = remap_coverage(&subtable.coverage, gid_map);
                *subtable.coverage = new_cov;
                let old_subs = subtable.substitute_glyph_ids.clone();
                subtable.substitute_glyph_ids = permute_vec(&old_subs, &perm)
                    .iter()
                    .map(|g| remap_gid16(*g, gid_map))
                    .collect();
                for cov in subtable.backtrack_coverages.iter_mut() {
                    let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                    **cov = new_cov;
                }
                for cov in subtable.lookahead_coverages.iter_mut() {
                    let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                    **cov = new_cov;
                }
            }
        }
    }
}

/// Remap a SingleSubst subtable. Always converts Format 1 (delta) to Format 2
/// (explicit mapping) because the delta may not be valid after remapping.
fn remap_single_subst(
    subtable: &mut write_fonts::tables::gsub::SingleSubst,
    gid_map: &BTreeMap<u32, u32>,
) {
    match subtable {
        write_fonts::tables::gsub::SingleSubst::Format1(f1) => {
            // Convert to Format 2: compute explicit output for each input
            let delta = f1.delta_glyph_id as i32;
            let old_gids = coverage_gids(&f1.coverage);
            let mut entries: Vec<(u32, u32)> = old_gids
                .iter()
                .map(|&old_input| {
                    let old_output = (old_input as i32 + delta) as u32;
                    let new_input = gid_map.get(&old_input).copied().unwrap_or(0);
                    let new_output = gid_map.get(&old_output).copied().unwrap_or(0);
                    (new_input, new_output)
                })
                .collect();
            // Sort by new input GID
            entries.sort_by_key(|&(input, _)| input);
            let glyph_array: Vec<GlyphId16> = entries
                .iter()
                .map(|&(g, _)| GlyphId16::new(g as u16))
                .collect();
            let substitute_ids: Vec<GlyphId16> = entries
                .iter()
                .map(|&(_, g)| GlyphId16::new(g as u16))
                .collect();
            *subtable = write_fonts::tables::gsub::SingleSubst::format_2(
                CoverageTable::format_1(glyph_array),
                substitute_ids,
            );
        }
        write_fonts::tables::gsub::SingleSubst::Format2(f2) => {
            let (new_cov, perm) = remap_coverage(&f2.coverage, gid_map);
            *f2.coverage = new_cov;
            let old_subs = f2.substitute_glyph_ids.clone();
            f2.substitute_glyph_ids = permute_vec(&old_subs, &perm)
                .iter()
                .map(|g| remap_gid16(*g, gid_map))
                .collect();
        }
    }
}

/// Remap a GSUB Extension subtable by accessing the inner subtable directly.
fn remap_gsub_extension(
    ext: &mut write_fonts::tables::gsub::ExtensionSubtable,
    gid_map: &BTreeMap<u32, u32>,
) {
    use write_fonts::tables::gsub::ExtensionSubtable;
    match ext {
        ExtensionSubtable::Single(l) => {
            remap_single_subst(&mut l.extension, gid_map);
        }
        ExtensionSubtable::Multiple(l) => {
            let sub = &mut *l.extension;
            let (new_cov, perm) = remap_coverage(&sub.coverage, gid_map);
            *sub.coverage = new_cov;
            let old_sequences: Vec<_> = sub.sequences.clone();
            sub.sequences = permute_vec(&old_sequences, &perm);
            for seq in sub.sequences.iter_mut() {
                for gid in seq.substitute_glyph_ids.iter_mut() {
                    *gid = remap_gid16(*gid, gid_map);
                }
            }
        }
        ExtensionSubtable::Alternate(l) => {
            let sub = &mut *l.extension;
            let (new_cov, perm) = remap_coverage(&sub.coverage, gid_map);
            *sub.coverage = new_cov;
            let old_sets: Vec<_> = sub.alternate_sets.clone();
            sub.alternate_sets = permute_vec(&old_sets, &perm);
            for alt_set in sub.alternate_sets.iter_mut() {
                for gid in alt_set.alternate_glyph_ids.iter_mut() {
                    *gid = remap_gid16(*gid, gid_map);
                }
            }
        }
        ExtensionSubtable::Ligature(l) => {
            let sub = &mut *l.extension;
            let (new_cov, perm) = remap_coverage(&sub.coverage, gid_map);
            *sub.coverage = new_cov;
            let old_sets: Vec<_> = sub.ligature_sets.clone();
            sub.ligature_sets = permute_vec(&old_sets, &perm);
            for lig_set in sub.ligature_sets.iter_mut() {
                for lig in lig_set.ligatures.iter_mut() {
                    lig.ligature_glyph = remap_gid16(lig.ligature_glyph, gid_map);
                    for gid in lig.component_glyph_ids.iter_mut() {
                        *gid = remap_gid16(*gid, gid_map);
                    }
                }
            }
        }
        ExtensionSubtable::Contextual(l) => {
            remap_sequence_context(&mut l.extension, gid_map);
        }
        ExtensionSubtable::ChainContextual(l) => {
            remap_chain_context(&mut l.extension, gid_map);
        }
        ExtensionSubtable::Reverse(l) => {
            let sub = &mut *l.extension;
            let (new_cov, perm) = remap_coverage(&sub.coverage, gid_map);
            *sub.coverage = new_cov;
            let old_subs = sub.substitute_glyph_ids.clone();
            sub.substitute_glyph_ids = permute_vec(&old_subs, &perm)
                .iter()
                .map(|g| remap_gid16(*g, gid_map))
                .collect();
            for cov in sub.backtrack_coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
            for cov in sub.lookahead_coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
        }
    }
}

/// Remap a SequenceContext (used in GSUB type 5 / GPOS type 7).
fn remap_sequence_context(
    ctx: &mut write_fonts::tables::layout::SequenceContext,
    gid_map: &BTreeMap<u32, u32>,
) {
    use write_fonts::tables::layout::SequenceContext;
    match ctx {
        SequenceContext::Format1(f1) => {
            let (new_cov, perm) = remap_coverage(&f1.coverage, gid_map);
            *f1.coverage = new_cov;
            let old_sets: Vec<_> = f1.seq_rule_sets.clone();
            f1.seq_rule_sets = permute_vec(&old_sets, &perm);
            for rule_set_opt in f1.seq_rule_sets.iter_mut() {
                if let Some(rule_set) = rule_set_opt.as_deref_mut() {
                    for rule in rule_set.seq_rules.iter_mut() {
                        for gid in rule.input_sequence.iter_mut() {
                            *gid = remap_gid16(*gid, gid_map);
                        }
                    }
                }
            }
        }
        SequenceContext::Format2(f2) => {
            let (new_cov, _) = remap_coverage(&f2.coverage, gid_map);
            *f2.coverage = new_cov;
            *f2.class_def = remap_class_def(&f2.class_def, gid_map);
        }
        SequenceContext::Format3(f3) => {
            for cov in f3.coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
        }
    }
}

/// Remap a ChainedSequenceContext (used in GSUB type 6 / GPOS type 8).
fn remap_chain_context(
    ctx: &mut write_fonts::tables::layout::ChainedSequenceContext,
    gid_map: &BTreeMap<u32, u32>,
) {
    use write_fonts::tables::layout::ChainedSequenceContext;
    match ctx {
        ChainedSequenceContext::Format1(f1) => {
            let (new_cov, perm) = remap_coverage(&f1.coverage, gid_map);
            *f1.coverage = new_cov;
            let old_sets: Vec<_> = f1.chained_seq_rule_sets.clone();
            f1.chained_seq_rule_sets = permute_vec(&old_sets, &perm);
            for rule_set_opt in f1.chained_seq_rule_sets.iter_mut() {
                if let Some(rule_set) = rule_set_opt.as_deref_mut() {
                    for rule in rule_set.chained_seq_rules.iter_mut() {
                        for gid in rule.backtrack_sequence.iter_mut() {
                            *gid = remap_gid16(*gid, gid_map);
                        }
                        for gid in rule.input_sequence.iter_mut() {
                            *gid = remap_gid16(*gid, gid_map);
                        }
                        for gid in rule.lookahead_sequence.iter_mut() {
                            *gid = remap_gid16(*gid, gid_map);
                        }
                    }
                }
            }
        }
        ChainedSequenceContext::Format2(f2) => {
            let (new_cov, _) = remap_coverage(&f2.coverage, gid_map);
            *f2.coverage = new_cov;
            *f2.backtrack_class_def = remap_class_def(&f2.backtrack_class_def, gid_map);
            *f2.input_class_def = remap_class_def(&f2.input_class_def, gid_map);
            *f2.lookahead_class_def = remap_class_def(&f2.lookahead_class_def, gid_map);
        }
        ChainedSequenceContext::Format3(f3) => {
            for cov in f3.backtrack_coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
            for cov in f3.input_coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
            for cov in f3.lookahead_coverages.iter_mut() {
                let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
                **cov = new_cov;
            }
        }
    }
}

/// Remap the GPOS table. Returns None if the font has no GPOS table.
fn remap_gpos_table(base_font: &FontRef, gid_map: &BTreeMap<u32, u32>) -> Option<Vec<u8>> {
    let gpos_data = base_font.data_for_tag(read_fonts::types::Tag::new(b"GPOS"))?;
    let parsed = read_fonts::tables::gpos::Gpos::read(gpos_data).ok()?;
    let mut gpos: write_fonts::tables::gpos::Gpos = parsed.to_owned_table();

    for lookup in gpos.lookup_list.lookups.iter_mut() {
        remap_position_lookup(lookup, gid_map);
    }

    write_fonts::dump_table(&gpos).ok()
}

/// Remap all GID references in a GPOS position lookup.
fn remap_position_lookup(lookup: &mut PositionLookup, gid_map: &BTreeMap<u32, u32>) {
    match lookup {
        PositionLookup::Single(l) => {
            for subtable in l.subtables.iter_mut() {
                match subtable.as_mut() {
                    write_fonts::tables::gpos::SinglePos::Format1(f1) => {
                        let (new_cov, _) = remap_coverage(&f1.coverage, gid_map);
                        *f1.coverage = new_cov;
                    }
                    write_fonts::tables::gpos::SinglePos::Format2(f2) => {
                        let (new_cov, perm) = remap_coverage(&f2.coverage, gid_map);
                        *f2.coverage = new_cov;
                        let old_records = f2.value_records.clone();
                        f2.value_records = permute_vec(&old_records, &perm);
                    }
                }
            }
        }
        PositionLookup::Pair(l) => {
            for subtable in l.subtables.iter_mut() {
                match subtable.as_mut() {
                    write_fonts::tables::gpos::PairPos::Format1(f1) => {
                        let (new_cov, perm) = remap_coverage(&f1.coverage, gid_map);
                        *f1.coverage = new_cov;
                        let old_sets: Vec<_> = f1.pair_sets.clone();
                        f1.pair_sets = permute_vec(&old_sets, &perm);
                        // Remap secondGlyph in PairValueRecords and sort by secondGlyph
                        for pair_set in f1.pair_sets.iter_mut() {
                            for pvr in pair_set.pair_value_records.iter_mut() {
                                pvr.second_glyph = remap_gid16(pvr.second_glyph, gid_map);
                            }
                            pair_set
                                .pair_value_records
                                .sort_by_key(|r| r.second_glyph.to_u16());
                        }
                    }
                    write_fonts::tables::gpos::PairPos::Format2(f2) => {
                        let (new_cov, _) = remap_coverage(&f2.coverage, gid_map);
                        *f2.coverage = new_cov;
                        *f2.class_def1 = remap_class_def(&f2.class_def1, gid_map);
                        *f2.class_def2 = remap_class_def(&f2.class_def2, gid_map);
                    }
                }
            }
        }
        PositionLookup::Cursive(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_cov, perm) = remap_coverage(&subtable.coverage, gid_map);
                *subtable.coverage = new_cov;
                let old_records = subtable.entry_exit_record.clone();
                subtable.entry_exit_record = permute_vec(&old_records, &perm);
            }
        }
        PositionLookup::MarkToBase(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_mark_cov, mark_perm) = remap_coverage(&subtable.mark_coverage, gid_map);
                *subtable.mark_coverage = new_mark_cov;
                let (new_base_cov, base_perm) = remap_coverage(&subtable.base_coverage, gid_map);
                *subtable.base_coverage = new_base_cov;
                // Permute mark array
                let old_marks = subtable.mark_array.mark_records.clone();
                subtable.mark_array.mark_records = permute_vec(&old_marks, &mark_perm);
                // Permute base array
                let old_bases = subtable.base_array.base_records.clone();
                subtable.base_array.base_records = permute_vec(&old_bases, &base_perm);
            }
        }
        PositionLookup::MarkToLig(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_mark_cov, mark_perm) = remap_coverage(&subtable.mark_coverage, gid_map);
                *subtable.mark_coverage = new_mark_cov;
                let (new_lig_cov, lig_perm) = remap_coverage(&subtable.ligature_coverage, gid_map);
                *subtable.ligature_coverage = new_lig_cov;
                let old_marks = subtable.mark_array.mark_records.clone();
                subtable.mark_array.mark_records = permute_vec(&old_marks, &mark_perm);
                let old_ligs = subtable.ligature_array.ligature_attaches.clone();
                subtable.ligature_array.ligature_attaches = permute_vec(&old_ligs, &lig_perm);
            }
        }
        PositionLookup::MarkToMark(l) => {
            for subtable in l.subtables.iter_mut() {
                let (new_mark1_cov, mark1_perm) = remap_coverage(&subtable.mark1_coverage, gid_map);
                *subtable.mark1_coverage = new_mark1_cov;
                let (new_mark2_cov, mark2_perm) = remap_coverage(&subtable.mark2_coverage, gid_map);
                *subtable.mark2_coverage = new_mark2_cov;
                let old_marks1 = subtable.mark1_array.mark_records.clone();
                subtable.mark1_array.mark_records = permute_vec(&old_marks1, &mark1_perm);
                let old_marks2 = subtable.mark2_array.mark2_records.clone();
                subtable.mark2_array.mark2_records = permute_vec(&old_marks2, &mark2_perm);
            }
        }
        PositionLookup::Contextual(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_sequence_context(subtable.as_mut(), gid_map);
            }
        }
        PositionLookup::ChainContextual(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_chain_context(subtable.as_mut(), gid_map);
            }
        }
        PositionLookup::Extension(l) => {
            for subtable in l.subtables.iter_mut() {
                remap_gpos_extension(subtable.as_mut(), gid_map);
            }
        }
    }
}

/// Remap a GPOS Extension subtable by accessing the inner subtable directly.
fn remap_gpos_extension(
    ext: &mut write_fonts::tables::gpos::ExtensionSubtable,
    gid_map: &BTreeMap<u32, u32>,
) {
    use write_fonts::tables::gpos::ExtensionSubtable;
    match ext {
        ExtensionSubtable::Single(l) => match &mut *l.extension {
            write_fonts::tables::gpos::SinglePos::Format1(f1) => {
                let (new_cov, _) = remap_coverage(&f1.coverage, gid_map);
                *f1.coverage = new_cov;
            }
            write_fonts::tables::gpos::SinglePos::Format2(f2) => {
                let (new_cov, perm) = remap_coverage(&f2.coverage, gid_map);
                *f2.coverage = new_cov;
                let old_records = f2.value_records.clone();
                f2.value_records = permute_vec(&old_records, &perm);
            }
        },
        ExtensionSubtable::Pair(l) => match &mut *l.extension {
            write_fonts::tables::gpos::PairPos::Format1(f1) => {
                let (new_cov, perm) = remap_coverage(&f1.coverage, gid_map);
                *f1.coverage = new_cov;
                let old_sets: Vec<_> = f1.pair_sets.clone();
                f1.pair_sets = permute_vec(&old_sets, &perm);
                for pair_set in f1.pair_sets.iter_mut() {
                    for pvr in pair_set.pair_value_records.iter_mut() {
                        pvr.second_glyph = remap_gid16(pvr.second_glyph, gid_map);
                    }
                    pair_set
                        .pair_value_records
                        .sort_by_key(|r| r.second_glyph.to_u16());
                }
            }
            write_fonts::tables::gpos::PairPos::Format2(f2) => {
                let (new_cov, _) = remap_coverage(&f2.coverage, gid_map);
                *f2.coverage = new_cov;
                *f2.class_def1 = remap_class_def(&f2.class_def1, gid_map);
                *f2.class_def2 = remap_class_def(&f2.class_def2, gid_map);
            }
        },
        ExtensionSubtable::Cursive(l) => {
            let sub = &mut *l.extension;
            let (new_cov, perm) = remap_coverage(&sub.coverage, gid_map);
            *sub.coverage = new_cov;
            let old_records = sub.entry_exit_record.clone();
            sub.entry_exit_record = permute_vec(&old_records, &perm);
        }
        ExtensionSubtable::MarkToBase(l) => {
            let sub = &mut *l.extension;
            let (new_mark_cov, mark_perm) = remap_coverage(&sub.mark_coverage, gid_map);
            *sub.mark_coverage = new_mark_cov;
            let (new_base_cov, base_perm) = remap_coverage(&sub.base_coverage, gid_map);
            *sub.base_coverage = new_base_cov;
            let old_marks = sub.mark_array.mark_records.clone();
            sub.mark_array.mark_records = permute_vec(&old_marks, &mark_perm);
            let old_bases = sub.base_array.base_records.clone();
            sub.base_array.base_records = permute_vec(&old_bases, &base_perm);
        }
        ExtensionSubtable::MarkToLig(l) => {
            let sub = &mut *l.extension;
            let (new_mark_cov, mark_perm) = remap_coverage(&sub.mark_coverage, gid_map);
            *sub.mark_coverage = new_mark_cov;
            let (new_lig_cov, lig_perm) = remap_coverage(&sub.ligature_coverage, gid_map);
            *sub.ligature_coverage = new_lig_cov;
            let old_marks = sub.mark_array.mark_records.clone();
            sub.mark_array.mark_records = permute_vec(&old_marks, &mark_perm);
            let old_ligs = sub.ligature_array.ligature_attaches.clone();
            sub.ligature_array.ligature_attaches = permute_vec(&old_ligs, &lig_perm);
        }
        ExtensionSubtable::MarkToMark(l) => {
            let sub = &mut *l.extension;
            let (new_mark1_cov, mark1_perm) = remap_coverage(&sub.mark1_coverage, gid_map);
            *sub.mark1_coverage = new_mark1_cov;
            let (new_mark2_cov, mark2_perm) = remap_coverage(&sub.mark2_coverage, gid_map);
            *sub.mark2_coverage = new_mark2_cov;
            let old_marks1 = sub.mark1_array.mark_records.clone();
            sub.mark1_array.mark_records = permute_vec(&old_marks1, &mark1_perm);
            let old_marks2 = sub.mark2_array.mark2_records.clone();
            sub.mark2_array.mark2_records = permute_vec(&old_marks2, &mark2_perm);
        }
        ExtensionSubtable::Contextual(l) => {
            remap_sequence_context(&mut l.extension, gid_map);
        }
        ExtensionSubtable::ChainContextual(l) => {
            remap_chain_context(&mut l.extension, gid_map);
        }
    }
}

/// Remap the GDEF table. Returns None if the font has no GDEF table.
fn remap_gdef_table(base_font: &FontRef, gid_map: &BTreeMap<u32, u32>) -> Option<Vec<u8>> {
    let gdef_data = base_font.data_for_tag(read_fonts::types::Tag::new(b"GDEF"))?;
    let parsed = read_fonts::tables::gdef::Gdef::read(gdef_data).ok()?;
    let mut gdef: write_fonts::tables::gdef::Gdef = parsed.to_owned_table();

    // Remap GlyphClassDef
    if let Some(class_def) = gdef.glyph_class_def.as_deref_mut() {
        *class_def = remap_class_def(class_def, gid_map);
    }

    // Remap AttachList coverage
    if let Some(attach_list) = gdef.attach_list.as_deref_mut() {
        let (new_cov, perm) = remap_coverage(&attach_list.coverage, gid_map);
        *attach_list.coverage = new_cov;
        let old_points = attach_list.attach_points.clone();
        attach_list.attach_points = permute_vec(&old_points, &perm);
    }

    // Remap LigCaretList coverage
    if let Some(lig_caret_list) = gdef.lig_caret_list.as_deref_mut() {
        let (new_cov, perm) = remap_coverage(&lig_caret_list.coverage, gid_map);
        *lig_caret_list.coverage = new_cov;
        let old_glyphs = lig_caret_list.lig_glyphs.clone();
        lig_caret_list.lig_glyphs = permute_vec(&old_glyphs, &perm);
    }

    // Remap MarkAttachClassDef
    if let Some(class_def) = gdef.mark_attach_class_def.as_deref_mut() {
        *class_def = remap_class_def(class_def, gid_map);
    }

    // Remap MarkGlyphSetsDef coverages
    if let Some(mark_glyph_sets) = gdef.mark_glyph_sets_def.as_deref_mut() {
        for cov in mark_glyph_sets.coverages.iter_mut() {
            let (new_cov, _) = remap_coverage(cov.as_ref(), gid_map);
            **cov = new_cov;
        }
    }

    write_fonts::dump_table(&gdef).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ResolvedTtfFile;
    use std::sync::Arc;

    /// Group downloaded font data by weight/style and merge each group.
    ///
    /// `files` and `font_data` are parallel slices (same length, same order).
    /// Returns one `Arc<Vec<u8>>` per unique (weight, style) combination,
    /// in deterministic order (sorted by weight then style).
    fn merge_font_data(
        font_id: &str,
        files: &[ResolvedTtfFile],
        font_data: Vec<Arc<Vec<u8>>>,
    ) -> Result<Vec<Arc<Vec<u8>>>, FontsourceError> {
        assert_eq!(files.len(), font_data.len());

        let mut groups: BTreeMap<(u16, FontStyle), Vec<&[u8]>> = BTreeMap::new();
        for (file, data) in files.iter().zip(font_data.iter()) {
            groups
                .entry((file.weight, file.style))
                .or_default()
                .push(data.as_slice());
        }

        let mut merged = Vec::with_capacity(groups.len());
        for ((weight, style), subset_bytes) in &groups {
            let result = merge_subsets(font_id, *weight, *style, subset_bytes)?;
            merged.push(Arc::new(result));
        }

        Ok(merged)
    }

    #[test]
    fn test_single_subset_passthrough() {
        // A single subset should be returned unchanged
        let ttf_bytes = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
        let files = vec![ResolvedTtfFile {
            url: "test".to_string(),
            weight: 400,
            style: FontStyle::Normal,
        }];
        let font_data = vec![Arc::new(ttf_bytes.to_vec())];

        let result = merge_font_data("test-font", &files, font_data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_slice(), ttf_bytes.as_slice());
    }

    #[test]
    fn test_merge_two_subsets() {
        // Use two real TTF files as synthetic "subsets"
        let ttf1 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
        let ttf2 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Bold.ttf");

        let files = vec![
            ResolvedTtfFile {
                url: "latin".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
            ResolvedTtfFile {
                url: "latin-ext".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
        ];
        let font_data = vec![Arc::new(ttf1.to_vec()), Arc::new(ttf2.to_vec())];

        let result = merge_font_data("test-font", &files, font_data).unwrap();
        assert_eq!(result.len(), 1);

        // Verify the merged font is valid
        let merged_font = FontRef::new(&result[0]).unwrap();
        let maxp = merged_font.maxp().unwrap();

        // The merged font should have a non-trivial number of glyphs
        // (union of cmap entries from both fonts + composite components + .notdef)
        assert!(maxp.num_glyphs() > 1);
    }

    #[test]
    fn test_merge_invalid_font_returns_error() {
        let files = vec![
            ResolvedTtfFile {
                url: "bad".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
            ResolvedTtfFile {
                url: "bad2".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
        ];
        let font_data = vec![
            Arc::new(b"not a font".to_vec()),
            Arc::new(b"also not a font".to_vec()),
        ];

        let result = merge_font_data("test-font", &files, font_data);
        assert!(result.is_err());
        match result.unwrap_err() {
            FontsourceError::FontMerge { font_id, .. } => {
                assert_eq!(font_id, "test-font");
            }
            other => panic!("Expected FontMerge error, got: {other}"),
        }
    }

    #[test]
    fn test_merge_preserves_name_table() {
        let ttf1 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
        let ttf2 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Bold.ttf");

        let files = vec![
            ResolvedTtfFile {
                url: "latin".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
            ResolvedTtfFile {
                url: "latin-ext".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
        ];
        let font_data = vec![Arc::new(ttf1.to_vec()), Arc::new(ttf2.to_vec())];

        let result = merge_font_data("test-font", &files, font_data).unwrap();
        let merged = FontRef::new(&result[0]).unwrap();
        let base = FontRef::new(ttf1).unwrap();

        // Name table should be carried over from base
        let merged_name_data = merged
            .data_for_tag(read_fonts::types::Tag::new(b"name"))
            .unwrap();
        let base_name_data = base
            .data_for_tag(read_fonts::types::Tag::new(b"name"))
            .unwrap();
        assert_eq!(merged_name_data.as_bytes(), base_name_data.as_bytes());
    }

    #[test]
    fn test_different_weight_style_groups() {
        let ttf1 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Regular.ttf");
        let ttf2 = include_bytes!("../../vl-convert-rs/tests/fonts/matter/Matter-Bold.ttf");

        let files = vec![
            ResolvedTtfFile {
                url: "latin-400".to_string(),
                weight: 400,
                style: FontStyle::Normal,
            },
            ResolvedTtfFile {
                url: "latin-700".to_string(),
                weight: 700,
                style: FontStyle::Normal,
            },
        ];
        let font_data = vec![Arc::new(ttf1.to_vec()), Arc::new(ttf2.to_vec())];

        let result = merge_font_data("test-font", &files, font_data).unwrap();
        // Two different weight/style groups -> two outputs
        assert_eq!(result.len(), 2);

        // Both should be valid fonts
        FontRef::new(&result[0]).unwrap();
        FontRef::new(&result[1]).unwrap();
    }
}
