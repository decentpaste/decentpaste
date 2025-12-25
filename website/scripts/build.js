#!/usr/bin/env node

/**
 * Production build script for DecentPaste website
 * Outputs minified HTML, CSS, and JS to the dist/ folder
 */

import { execSync } from 'child_process';
import { mkdirSync, rmSync, readFileSync, writeFileSync, copyFileSync, readdirSync, statSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { minify as minifyHTML } from 'html-minifier-terser';
import { minify as minifyJS } from 'terser';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const DIST = join(ROOT, 'dist');

// HTML minification options
const htmlMinifyOptions = {
  collapseWhitespace: true,
  removeComments: true,
  removeRedundantAttributes: true,
  removeScriptTypeAttributes: true,
  removeStyleLinkTypeAttributes: true,
  useShortDoctype: true,
  minifyCSS: true,
  minifyJS: true,
};

// Copy directory recursively
function copyDir(src, dest) {
  mkdirSync(dest, { recursive: true });
  for (const entry of readdirSync(src)) {
    const srcPath = join(src, entry);
    const destPath = join(dest, entry);
    if (statSync(srcPath).isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      copyFileSync(srcPath, destPath);
    }
  }
}

async function build() {
  console.log('🚀 Starting production build...\n');

  // 1. Clean dist folder
  console.log('🧹 Cleaning dist folder...');
  rmSync(DIST, { recursive: true, force: true });
  mkdirSync(DIST, { recursive: true });

  // 2. Build CSS with Tailwind
  console.log('🎨 Building CSS with Tailwind v4...');
  execSync('npx @tailwindcss/cli -i ./src/input.css -o ./dist/output.css --minify', {
    cwd: ROOT,
    stdio: 'inherit',
  });

  // 3. Minify HTML
  console.log('📄 Minifying HTML...');
  let html = readFileSync(join(ROOT, 'index.html'), 'utf-8');

  // Update CSS path for production (dist/output.css -> output.css)
  html = html.replace('href="dist/output.css"', 'href="output.css"');

  // Update JS path for production (script.js -> script.js - will be in same folder)
  // No change needed if script.js is referenced without path

  const minifiedHTML = await minifyHTML(html, htmlMinifyOptions);
  writeFileSync(join(DIST, 'index.html'), minifiedHTML);

  // 4. Minify JavaScript
  console.log('⚡ Minifying JavaScript...');
  const js = readFileSync(join(ROOT, 'script.js'), 'utf-8');
  const minifiedJS = await minifyJS(js, {
    compress: true,
    mangle: true,
  });
  writeFileSync(join(DIST, 'script.js'), minifiedJS.code);

  // 5. Copy assets
  console.log('📁 Copying assets...');
  copyDir(join(ROOT, 'assets'), join(DIST, 'assets'));

  // 6. Copy other static files
  console.log('📋 Copying static files...');
  const staticFiles = ['robots.txt', 'sitemap.xml', 'CNAME', 'downloads.json', 'privacy.html'];
  for (const file of staticFiles) {
    try {
      copyFileSync(join(ROOT, file), join(DIST, file));
    } catch {
      // File might not exist, skip silently
    }
  }

  // 7. Calculate sizes
  const cssSize = (statSync(join(DIST, 'output.css')).size / 1024).toFixed(1);
  const htmlSize = (statSync(join(DIST, 'index.html')).size / 1024).toFixed(1);
  const jsSize = (statSync(join(DIST, 'script.js')).size / 1024).toFixed(1);

  console.log('\n✅ Build complete!\n');
  console.log('📦 Output sizes:');
  console.log(`   HTML: ${htmlSize} KB`);
  console.log(`   CSS:  ${cssSize} KB`);
  console.log(`   JS:   ${jsSize} KB`);
  console.log(`\n📂 Output folder: dist/`);
  console.log('🌐 Preview with: yarn preview');
}

build().catch((err) => {
  console.error('❌ Build failed:', err);
  process.exit(1);
});
