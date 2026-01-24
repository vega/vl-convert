#!/usr/bin/env node
/**
 * Validates bundled HTML files by loading them in headless Chrome,
 * checking for console errors, and taking screenshots.
 *
 * Usage: node validate-html.mjs <html-file> <output-png>
 *
 * Set CHROME_PATH environment variable to specify Chrome executable path.
 */

import puppeteer from 'puppeteer-core';
import path from 'path';

const args = process.argv.slice(2);
if (args.length < 2) {
    console.error('Usage: node validate-html.mjs <html-file> <output-png>');
    process.exit(1);
}

const htmlFile = args[0];
const outputPng = args[1];

// Find Chrome executable
const chromePath = process.env.CHROME_PATH || '/usr/bin/google-chrome-stable';
console.log(`Using Chrome at: ${chromePath}`);

const errors = [];

async function main() {
    const browser = await puppeteer.launch({
        headless: true,
        executablePath: chromePath,
        args: ['--no-sandbox', '--disable-setuid-sandbox']
    });

    const page = await browser.newPage();

    // Capture console errors
    page.on('console', msg => {
        if (msg.type() === 'error') {
            errors.push(msg.text());
            console.error(`Console error: ${msg.text()}`);
        }
    });

    // Capture page errors (uncaught exceptions)
    page.on('pageerror', err => {
        errors.push(err.message);
        console.error(`Page error: ${err.message}`);
    });

    // Load the HTML file
    const fileUrl = `file://${path.resolve(htmlFile)}`;
    console.log(`Loading: ${fileUrl}`);

    await page.goto(fileUrl, {
        waitUntil: 'networkidle0',
        timeout: 30000
    });

    // Wait a bit for Vega to render
    await page.waitForSelector('canvas, svg', { timeout: 10000 }).catch(() => {
        console.log('Warning: No canvas or svg element found');
    });

    // Additional wait for rendering to complete
    await new Promise(resolve => setTimeout(resolve, 1000));

    // Take screenshot
    await page.screenshot({
        path: outputPng,
        fullPage: true
    });
    console.log(`Screenshot saved: ${outputPng}`);

    await browser.close();

    // Exit with error if there were console errors
    if (errors.length > 0) {
        console.error(`\nFound ${errors.length} console error(s):`);
        errors.forEach((err, i) => console.error(`  ${i + 1}. ${err}`));
        process.exit(1);
    }

    console.log('Validation passed - no console errors');
}

main().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
