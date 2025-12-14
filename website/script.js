/**
 * DecentPaste Landing Page Scripts
 * Platform detection, dark mode, mobile menu, and scroll animations
 */

// =============================================================================
// Platform Detection
// =============================================================================

const platformConfig = {
  windows: {
    name: 'Windows',
    icon: `<svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_x64-setup.exe',
  },
  macos: {
    name: 'macOS',
    icon: `<svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_x64.dmg',
  },
  linux: {
    name: 'Linux',
    icon: `<svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139z"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_amd64.AppImage',
  },
  android: {
    name: 'Android',
    icon: `<svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor"><path d="M17.523 15.3414c-.5511 0-.9993-.4486-.9993-.9997s.4483-.9993.9993-.9993c.5511 0 .9993.4483.9993.9993.0001.5511-.4482.9997-.9993.9997m-11.046 0c-.5511 0-.9993-.4486-.9993-.9997s.4482-.9993.9993-.9993c.5511 0 .9993.4483.9993.9993 0 .5511-.4483.9997-.9993.9997m11.4045-6.02l1.9973-3.4592a.416.416 0 00-.1521-.5676.416.416 0 00-.5676.1521l-2.0223 3.503C15.5902 8.2439 13.8533 7.8508 12 7.8508s-3.5902.3931-5.1367 1.0989L4.841 5.4467a.4161.4161 0 00-.5677-.1521.4157.4157 0 00-.1521.5676l1.9973 3.4592C2.6889 11.1867.3432 14.6589 0 18.761h24c-.3435-4.1021-2.6892-7.5743-6.1185-9.4396"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste.apk',
  },
  ios: {
    name: 'iOS',
    icon: `<svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>`,
    downloadUrl: '#downloads', // Coming soon
  },
  unknown: {
    name: 'Your Platform',
    icon: `<svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/></svg>`,
    downloadUrl: '#downloads',
  },
};

/**
 * Detect the user's operating system
 */
function detectPlatform() {
  const userAgent = navigator.userAgent.toLowerCase();
  const platform = navigator.platform?.toLowerCase() || '';

  // iOS detection (must come before macOS - iPad can report as Mac)
  if (
    /ipad|iphone|ipod/.test(userAgent) ||
    (platform === 'macintel' && navigator.maxTouchPoints > 1)
  ) {
    return 'ios';
  }

  // Android detection
  if (/android/.test(userAgent)) {
    return 'android';
  }

  // Windows detection
  if (/win/.test(platform) || /windows/.test(userAgent)) {
    return 'windows';
  }

  // macOS detection
  if (/mac/.test(platform) && !/iphone|ipad|ipod/.test(userAgent)) {
    return 'macos';
  }

  // Linux detection
  if (/linux/.test(platform) && !/android/.test(userAgent)) {
    return 'linux';
  }

  return 'unknown';
}

/**
 * Update the hero download button based on detected platform
 */
function updateHeroButton() {
  const platform = detectPlatform();
  const config = platformConfig[platform];

  const btn = document.getElementById('primary-download');
  const iconEl = document.getElementById('platform-icon');
  const nameEl = document.getElementById('platform-name');

  if (btn && config && iconEl && nameEl) {
    iconEl.innerHTML = config.icon;
    nameEl.textContent = config.name;
    btn.href = config.downloadUrl;

    // Highlight the matching download card
    const downloadCard = document.getElementById(`download-${platform}`);
    if (downloadCard) {
      downloadCard.classList.add(
        'ring-2',
        'ring-teal-500',
        'ring-offset-2',
        'dark:ring-offset-gray-950'
      );

      // Add "Recommended" badge
      const badge = document.createElement('div');
      badge.className =
        'absolute -top-3 left-1/2 -translate-x-1/2 px-3 py-1 bg-teal-500 text-white text-xs font-semibold rounded-full';
      badge.textContent = 'Recommended';
      downloadCard.style.position = 'relative';
      downloadCard.appendChild(badge);
    }
  }
}

// =============================================================================
// Dark Mode
// =============================================================================

/**
 * Initialize dark mode based on saved preference or system setting
 */
function initDarkMode() {
  const html = document.documentElement;
  const savedTheme = localStorage.getItem('theme');
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

  if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
    html.classList.add('dark');
  } else {
    html.classList.remove('dark');
  }
}

/**
 * Toggle dark mode and save preference
 */
function toggleDarkMode() {
  const html = document.documentElement;
  html.classList.toggle('dark');
  localStorage.setItem('theme', html.classList.contains('dark') ? 'dark' : 'light');
}

// =============================================================================
// Mobile Menu
// =============================================================================

/**
 * Initialize mobile menu functionality
 */
function initMobileMenu() {
  const menuBtn = document.getElementById('mobile-menu-btn');
  const mobileMenu = document.getElementById('mobile-menu');
  const menuIconOpen = document.getElementById('menu-icon-open');
  const menuIconClose = document.getElementById('menu-icon-close');

  if (!menuBtn || !mobileMenu) return;

  menuBtn.addEventListener('click', () => {
    const isOpen = !mobileMenu.classList.contains('hidden');

    if (isOpen) {
      mobileMenu.classList.add('hidden');
      menuIconOpen?.classList.remove('hidden');
      menuIconClose?.classList.add('hidden');
    } else {
      mobileMenu.classList.remove('hidden');
      menuIconOpen?.classList.add('hidden');
      menuIconClose?.classList.remove('hidden');
    }
  });

  // Close menu when clicking a link
  mobileMenu.querySelectorAll('a').forEach((link) => {
    link.addEventListener('click', () => {
      mobileMenu.classList.add('hidden');
      menuIconOpen?.classList.remove('hidden');
      menuIconClose?.classList.add('hidden');
    });
  });
}

// =============================================================================
// Navbar Background
// =============================================================================

/**
 * Update navbar background on scroll
 */
function initNavbarScroll() {
  const navbar = document.getElementById('navbar');
  if (!navbar) return;

  const updateNavbar = () => {
    if (window.scrollY > 50) {
      navbar.classList.add(
        'bg-white/90',
        'dark:bg-gray-950/90',
        'backdrop-blur-lg',
        'shadow-sm',
        'border-b',
        'border-gray-200/50',
        'dark:border-gray-800/50'
      );
    } else {
      navbar.classList.remove(
        'bg-white/90',
        'dark:bg-gray-950/90',
        'backdrop-blur-lg',
        'shadow-sm',
        'border-b',
        'border-gray-200/50',
        'dark:border-gray-800/50'
      );
    }
  };

  window.addEventListener('scroll', updateNavbar, { passive: true });
  updateNavbar(); // Initial check
}

// =============================================================================
// Scroll Animations
// =============================================================================

/**
 * Initialize fade-in animations on scroll
 */
function initScrollAnimations() {
  const observerOptions = {
    threshold: 0.1,
    rootMargin: '0px 0px -50px 0px',
  };

  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add('visible');
        observer.unobserve(entry.target); // Only animate once
      }
    });
  }, observerOptions);

  // Observe all elements with fade-in-up class
  document.querySelectorAll('.fade-in-up').forEach((el) => {
    observer.observe(el);
  });
}

// =============================================================================
// Event Listeners
// =============================================================================

document.addEventListener('DOMContentLoaded', () => {
  // Initialize all features
  initDarkMode();
  updateHeroButton();
  initMobileMenu();
  initNavbarScroll();
  initScrollAnimations();

  // Dark mode toggle buttons
  const themeToggle = document.getElementById('theme-toggle');
  const mobileThemeToggle = document.getElementById('mobile-theme-toggle');

  themeToggle?.addEventListener('click', toggleDarkMode);
  mobileThemeToggle?.addEventListener('click', toggleDarkMode);

  // Listen for system theme changes
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    if (!localStorage.getItem('theme')) {
      if (e.matches) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    }
  });
});

// Prevent flash of unstyled content
initDarkMode();
