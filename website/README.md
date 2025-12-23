# DecentPaste Website

A beautiful, modern landing page for DecentPaste - the cross-platform clipboard sharing app.

## Features

- **Static Site** - Pure HTML/CSS/JS, no server required
- **Platform Detection** - Automatically suggests the right download for the visitor's OS
- **Dark Mode** - Toggle + system preference detection
- **Fully Responsive** - Mobile-first design
- **Accessible** - Semantic HTML, keyboard navigation, reduced motion support
- **Fast** - No framework overhead, CDN-loaded Tailwind CSS

## Tech Stack

- HTML5
- Tailwind CSS (via CDN)
- Vanilla JavaScript
- Inter font (Google Fonts)

## Local Development

Simply open `index.html` in your browser, or use a local server:

```bash
# Using Python
python -m http.server 8000

# Using Node.js (npx)
npx serve .

# Using PHP
php -S localhost:8000
```

Then open http://localhost:8000

## Deployment

This is a static site - just upload the files to any static hosting provider:

### GitHub Pages (Free)

1. Push the `website/` folder to your repo
2. Go to Settings → Pages
3. Set source to your branch and `/website` folder
4. Your site will be live at `https://username.github.io/repo/`

Or use a separate branch:

```bash
# Create gh-pages branch with website contents
git subtree push --prefix website origin gh-pages
```

### Netlify (Free)

1. Drag and drop the `website/` folder to [Netlify Drop](https://app.netlify.com/drop)
2. Or connect your Git repo and set publish directory to `website/`

### Cloudflare Pages (Free)

1. Connect your Git repo
2. Set build command: (leave empty)
3. Set output directory: `website/`

### Vercel (Free)

1. Connect your Git repo
2. Set root directory: `website/`
3. Set framework preset: `Other`

### Custom Domain

After deploying, configure your DNS:

- Add an `A` record pointing to your host's IP
- Or add a `CNAME` record pointing to your host's domain

## File Structure

```
website/
├── index.html          # Main landing page
├── styles.css          # Custom animations and overrides
├── script.js           # Platform detection, dark mode, interactions
├── assets/
│   ├── logo.png        # App icon (512x512)
│   ├── logo_light.svg  # SVG logo for light mode
│   ├── logo_dark.svg   # SVG logo for dark mode
│   ├── logo_white.svg  # White SVG logo variant
│   ├── logo_black.svg  # Black SVG logo variant
│   ├── favicon.ico     # Browser favicon
│   ├── favicon-32x32.png
│   └── apple-touch-icon.png
└── README.md           # This file
```

## Customization

### Update Download Links

Edit the `platformConfig` object in `script.js`:

```javascript
const platformConfig = {
  windows: {
    downloadUrl: 'https://github.com/your-org/your-repo/releases/latest/download/App.exe',
  },
  // ...
};
```

### Change Colors

Modify the Tailwind config in `index.html`:

```html
<script>
  tailwind.config = {
    theme: {
      extend: {
        colors: {
          teal: {
            /* your colors */
          },
          orange: {
            /* your colors */
          },
        },
      },
    },
  };
</script>
```

### Add Screenshots

1. Add images to `assets/screenshots/`
2. Insert them in the hero or a new section in `index.html`

## Performance Tips

- Images are lazy-loaded
- CSS animations respect `prefers-reduced-motion`
- Tailwind CSS is loaded from CDN with caching
- No JavaScript framework overhead

## Browser Support

- Chrome 80+
- Firefox 75+
- Safari 13+
- Edge 80+
- iOS Safari 13+
- Chrome for Android 80+

## License

Apache-2.0 license - see the main repository for details.
