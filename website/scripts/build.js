#!/usr/bin/env node

/**
 * Production build script for DecentPaste website
 * Outputs minified HTML, CSS, and JS to the dist/ folder
 */

import { execSync } from 'child_process';
import { mkdirSync, rmSync, readFileSync, writeFileSync, copyFileSync, readdirSync, statSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { minify as minifyHTML } from 'html-minifier-terser';
import { minify as minifyJS } from 'terser';
import { marked } from 'marked';
import { markedHighlight } from 'marked-highlight';
import hljs from 'highlight.js';
import matter from 'gray-matter';

// Configure marked with syntax highlighting
marked.use(
  markedHighlight({
    langPrefix: 'hljs language-',
    highlight(code, lang) {
      const language = hljs.getLanguage(lang) ? lang : 'plaintext';
      return hljs.highlight(code, { language }).value;
    },
  }),
);

/**
 * Escape HTML special characters for safe insertion into HTML attributes/content
 */
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Escape string for safe insertion into JSON (JSON-LD structured data)
 */
function escapeJson(str) {
  if (!str) return '';
  return str
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/\n/g, '\\n')
    .replace(/\r/g, '\\r')
    .replace(/\t/g, '\\t');
}

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const DIST = join(ROOT, 'dist');

// Site configuration
const SITE_URL = 'https://decentpaste.com';

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

/**
 * Load an HTML template from src/templates/
 */
function loadTemplate(name) {
  return readFileSync(join(ROOT, 'src', 'templates', `${name}.html`), 'utf-8');
}

/**
 * Parse date string as UTC to avoid timezone shifts
 * Input: "2024-01-15" -> Date object at UTC midnight
 */
function parseDate(dateInput) {
  // If already a Date, use it; otherwise parse as UTC
  if (dateInput instanceof Date) return dateInput;
  // Append T00:00:00Z to force UTC interpretation
  return new Date(dateInput + 'T00:00:00Z');
}

/**
 * Format a date string for display (e.g., "January 15, 2024")
 */
function formatDate(dateInput) {
  const date = parseDate(dateInput);
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    timeZone: 'UTC',
  });
}

/**
 * Get ISO date string for datetime attributes
 */
function getISODate(dateInput) {
  const date = parseDate(dateInput);
  return date.toISOString().split('T')[0];
}

/**
 * Read all markdown posts from blog/posts/ and parse frontmatter
 * Returns array of posts sorted by date (newest first)
 */
function getAllPosts() {
  const postsDir = join(ROOT, 'blog', 'posts');

  if (!existsSync(postsDir)) {
    return [];
  }

  const files = readdirSync(postsDir).filter((f) => f.endsWith('.md'));

  const posts = files.map((filename) => {
    const filePath = join(postsDir, filename);
    const content = readFileSync(filePath, 'utf-8');
    const { data: frontmatter, content: markdown } = matter(content);

    // Validate required frontmatter fields
    if (!frontmatter.title) {
      throw new Error(`Missing 'title' in frontmatter: ${filename}`);
    }
    if (!frontmatter.date) {
      throw new Error(`Missing 'date' in frontmatter: ${filename}`);
    }
    if (isNaN(new Date(frontmatter.date).getTime())) {
      throw new Error(`Invalid 'date' in frontmatter: ${filename}`);
    }

    // Generate slug from filename (remove date prefix and .md extension)
    // e.g., "2024-01-15-my-post.md" -> "my-post"
    // Sanitize to only allow URL-safe characters (alphanumeric and hyphens)
    const slug = filename
      .replace(/^\d{4}-\d{2}-\d{2}-/, '')
      .replace('.md', '')
      .toLowerCase()
      .replace(/[^a-z0-9-]/g, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '');

    return {
      slug,
      filename,
      frontmatter,
      markdown,
      html: marked(markdown),
    };
  });

  // Sort by date (newest first)
  return posts.sort((a, b) => new Date(b.frontmatter.date) - new Date(a.frontmatter.date));
}

/**
 * Build a single blog post HTML from template
 */
function buildBlogPost(post, template) {
  const { frontmatter, html, slug } = post;

  const title = frontmatter.title || 'Untitled';
  const description = frontmatter.description || '';

  return template
    .replace(/\{\{title\}\}/g, escapeHtml(title))
    .replace(/\{\{titleJson\}\}/g, escapeJson(title))
    .replace(/\{\{description\}\}/g, escapeHtml(description))
    .replace(/\{\{descriptionJson\}\}/g, escapeJson(description))
    .replace(/\{\{slug\}\}/g, slug)
    .replace(/\{\{date\}\}/g, formatDate(frontmatter.date))
    .replace(/\{\{dateISO\}\}/g, getISODate(frontmatter.date))
    .replace(/\{\{content\}\}/g, html);
}

/**
 * Build the blog index page with all posts
 */
function buildBlogIndex(posts, template) {
  if (posts.length === 0) {
    return template
      .replace('{{posts}}', '')
      .replace('{{emptyState}}', '<div class="blog-empty-state"><p>No posts yet. Check back soon!</p></div>');
  }

  const postsHtml = posts
    .map((post) => {
      const title = escapeHtml(post.frontmatter.title || 'Untitled');
      const description = escapeHtml(post.frontmatter.description || '');
      return `
    <a href="/blog/${post.slug}/" class="blog-card">
      <time class="blog-card-date" datetime="${getISODate(post.frontmatter.date)}">
        ${formatDate(post.frontmatter.date)}
      </time>
      <h2 class="blog-card-title">${title}</h2>
      <p class="blog-card-description">${description}</p>
    </a>
  `;
    })
    .join('\n');

  return template.replace('{{posts}}', postsHtml).replace('{{emptyState}}', '');
}

/**
 * Generate RSS feed XML for blog posts
 */
function generateRSSFeed(posts) {
  const feedUrl = `${SITE_URL}/blog/feed.xml`;
  const buildDate = new Date().toUTCString();

  const items = posts
    .map((post) => {
      const postUrl = `${SITE_URL}/blog/${post.slug}/`;
      const pubDate = parseDate(post.frontmatter.date).toUTCString();

      return `    <item>
      <title><![CDATA[${post.frontmatter.title || 'Untitled'}]]></title>
      <link>${postUrl}</link>
      <guid isPermaLink="true">${postUrl}</guid>
      <description><![CDATA[${post.frontmatter.description || ''}]]></description>
      <pubDate>${pubDate}</pubDate>
    </item>`;
    })
    .join('\n');

  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>DecentPaste Blog</title>
    <link>${SITE_URL}/blog/</link>
    <description>Updates, tutorials, and announcements from the DecentPaste team.</description>
    <language>en-us</language>
    <lastBuildDate>${buildDate}</lastBuildDate>
    <atom:link href="${feedUrl}" rel="self" type="application/rss+xml"/>
${items}
  </channel>
</rss>`;
}

/**
 * Generate sitemap XML with all pages including blog posts
 */
function generateSitemap(posts) {
  const today = new Date().toISOString().split('T')[0];

  // Static pages
  const staticPages = [
    { loc: '/', priority: '1.0', changefreq: 'weekly' },
    { loc: '/privacy.html', priority: '0.5', changefreq: 'monthly' },
    { loc: '/support.html', priority: '0.6', changefreq: 'monthly' },
    { loc: '/blog/', priority: '0.9', changefreq: 'weekly' },
  ];

  const staticUrls = staticPages
    .map(
      (page) => `  <url>
    <loc>${SITE_URL}${page.loc}</loc>
    <lastmod>${today}</lastmod>
    <changefreq>${page.changefreq}</changefreq>
    <priority>${page.priority}</priority>
  </url>`,
    )
    .join('\n');

  // Blog post pages
  const postUrls = posts
    .map((post) => {
      const postDate = getISODate(post.frontmatter.date);
      return `  <url>
    <loc>${SITE_URL}/blog/${post.slug}/</loc>
    <lastmod>${postDate}</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.7</priority>
  </url>`;
    })
    .join('\n');

  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${staticUrls}
${postUrls}
</urlset>`;
}

/**
 * Build all blog content (posts + index)
 * Draft posts are built (accessible via direct URL) but hidden from index/RSS/sitemap
 */
async function buildBlog() {
  const allPosts = getAllPosts();
  const publishedPosts = allPosts.filter((p) => !p.frontmatter.draft);
  const blogDist = join(DIST, 'blog');

  // Create blog output directory
  mkdirSync(blogDist, { recursive: true });

  // Load templates
  const postTemplate = loadTemplate('blog-post');
  const indexTemplate = loadTemplate('blog-index');

  // Build ALL posts (including drafts - they're accessible via direct URL)
  for (const post of allPosts) {
    const postDir = join(blogDist, post.slug);
    mkdirSync(postDir, { recursive: true });

    let postHtml = buildBlogPost(post, postTemplate);
    postHtml = await minifyHTML(postHtml, htmlMinifyOptions);
    writeFileSync(join(postDir, 'index.html'), postHtml);
    const draftLabel = post.frontmatter.draft ? ' (draft)' : '';
    console.log(`   ✓ blog/${post.slug}/${draftLabel}`);
  }

  // Build blog index (published posts only)
  let indexHtml = buildBlogIndex(publishedPosts, indexTemplate);
  indexHtml = await minifyHTML(indexHtml, htmlMinifyOptions);
  writeFileSync(join(blogDist, 'index.html'), indexHtml);

  // Generate RSS feed (published posts only)
  const rssFeed = generateRSSFeed(publishedPosts);
  writeFileSync(join(blogDist, 'feed.xml'), rssFeed);
  console.log(`   ✓ blog/feed.xml`);

  // Generate sitemap (published posts only)
  const sitemap = generateSitemap(publishedPosts);
  writeFileSync(join(DIST, 'sitemap.xml'), sitemap);
  console.log(`   ✓ sitemap.xml`);

  const draftCount = allPosts.length - publishedPosts.length;
  return { total: allPosts.length, published: publishedPosts.length, drafts: draftCount };
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

  // 3. Minify HTML files
  console.log('📄 Minifying HTML...');

  const htmlFiles = ['index.html', 'privacy.html', 'support.html'];
  for (const file of htmlFiles) {
    try {
      let html = readFileSync(join(ROOT, file), 'utf-8');

      // Update CSS path for production (dist/output.css -> output.css)
      html = html.replace('href="dist/output.css"', 'href="output.css"');

      const minifiedHTML = await minifyHTML(html, htmlMinifyOptions);
      writeFileSync(join(DIST, file), minifiedHTML);
      console.log(`   ✓ ${file}`);
    } catch (err) {
      console.log(`   ⚠ ${file} not found, skipping`);
    }
  }

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

  // 6. Copy other static files (sitemap.xml is generated dynamically)
  console.log('📋 Copying static files...');
  const staticFiles = ['robots.txt', 'CNAME', 'downloads.json'];
  for (const file of staticFiles) {
    try {
      copyFileSync(join(ROOT, file), join(DIST, file));
    } catch {
      // File might not exist, skip silently
    }
  }

  // 7. Build blog
  console.log('📝 Building blog...');
  const { published, drafts } = await buildBlog();
  const draftInfo = drafts > 0 ? `, ${drafts} draft${drafts !== 1 ? 's' : ''}` : '';
  console.log(`   ✓ blog/index.html (${published} post${published !== 1 ? 's' : ''}${draftInfo})`);

  // 8. Calculate sizes
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
