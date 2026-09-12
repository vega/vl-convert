use std::process::Command;
use vl_convert_google_fonts::{google_fonts_cache_dir, ClientConfig};

#[test]
fn environment_config() {
    if let Ok(case) = std::env::var("VLC_TEST_FONT_CONFIG") {
        let config = ClientConfig::default();
        let expected_cache = match case.as_str() {
            "path" => Some(std::env::temp_dir().join("vlc font cache")),
            "disabled" => None,
            "default" => dirs::cache_dir().map(|p| p.join("vl-convert/google-fonts")),
            _ => panic!("Unknown test case: {case}"),
        };
        assert_eq!(config.cache_dir, expected_cache);
        assert_eq!(google_fonts_cache_dir(), expected_cache);
        assert_eq!(
            config.google_fonts_css2_url,
            if case == "default" {
                "https://fonts.googleapis.com/css2"
            } else {
                "http://127.0.0.1:12345/css2"
            }
        );
        return;
    }

    // Child processes keep environment changes isolated from parallel tests.
    for case in ["default", "path", "disabled"] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "environment_config"])
            .env("VLC_TEST_FONT_CONFIG", case)
            .env_remove("VLC_GOOGLE_FONTS_CACHE_DIR")
            .env_remove("VLC_GOOGLE_FONTS_CSS2_URL")
            .env("VL_CONVERT_FONT_CACHE_DIR", "ignored legacy cache")
            .env(
                "VL_CONVERT_GOOGLE_FONTS_CSS2_URL",
                "http://legacy.invalid/css2",
            );
        if case != "default" {
            command
                .env(
                    "VLC_GOOGLE_FONTS_CACHE_DIR",
                    if case == "path" {
                        std::env::temp_dir().join("vlc font cache").into_os_string()
                    } else {
                        "NoNe".into()
                    },
                )
                .env("VLC_GOOGLE_FONTS_CSS2_URL", "http://127.0.0.1:12345/css2");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{case}: {output:?}");
    }
}
