// Lucide icons integration
// Using the lucide library for proper SVG icon generation

import {
  Clipboard,
  Copy,
  Check,
  X,
  Settings,
  Users,
  History,
  Home,
  Wifi,
  WifiOff,
  Link,
  Unlink,
  Smartphone,
  Monitor,
  Trash2,
  RefreshCw,
  Loader,
  ArrowLeft,
  ArrowRight,
  Send,
  Share2,
  Download,
  Lock,
  Unlock,
  KeyRound,
  AlertTriangle,
  Shield,
  ChevronRight,
  Eye,
  EyeOff,
  createElement,
  type IconNode,
} from 'lucide';

// Map of icon names to their Lucide icon nodes
const iconMap: Record<string, IconNode> = {
  clipboard: Clipboard,
  copy: Copy,
  check: Check,
  x: X,
  settings: Settings,
  users: Users,
  history: History,
  home: Home,
  wifi: Wifi,
  wifiOff: WifiOff,
  link: Link,
  unlink: Unlink,
  smartphone: Smartphone,
  monitor: Monitor,
  trash: Trash2,
  refreshCw: RefreshCw,
  loader: Loader,
  arrowLeft: ArrowLeft,
  arrowRight: ArrowRight,
  send: Send,
  share: Share2,
  download: Download,
  lock: Lock,
  unlock: Unlock,
  key: KeyRound,
  alertTriangle: AlertTriangle,
  shield: Shield,
  chevronRight: ChevronRight,
  eye: Eye,
  eyeOff: EyeOff,
};

export type IconName = keyof typeof iconMap;

/**
 * Creates an SVG icon element and returns its HTML string.
 *
 * @param name - The name of the icon (e.g., 'clipboard', 'copy')
 * @param size - The size in pixels (default: 24)
 * @param className - Additional CSS classes to apply
 * @returns HTML string of the SVG element
 */
export function icon(name: IconName, size = 24, className = ''): string {
  const iconNode = iconMap[name];

  if (!iconNode) {
    console.warn(`Icon "${name}" not found`);
    return '';
  }

  // Create the SVG element using Lucide's createElement
  const svgElement = createElement(iconNode);

  // Set size attributes
  svgElement.setAttribute('width', String(size));
  svgElement.setAttribute('height', String(size));

  // Handle class merging
  const existingClasses = svgElement.getAttribute('class') || '';
  if (className) {
    svgElement.setAttribute('class', existingClasses ? `${existingClasses} ${className}` : className);
  }

  return svgElement.outerHTML;
}

/**
 * Creates an SVG icon DOM element directly.
 * Use this when you need to manipulate the element programmatically.
 *
 * @param name - The name of the icon
 * @param size - The size in pixels (default: 24)
 * @param className - Additional CSS classes to apply
 * @returns SVGElement or null if icon not found
 */
export function iconElement(name: IconName, size = 24, className = ''): SVGElement | null {
  const iconNode = iconMap[name];

  if (!iconNode) {
    console.warn(`Icon "${name}" not found`);
    return null;
  }

  const svgElement = createElement(iconNode);
  svgElement.setAttribute('width', String(size));
  svgElement.setAttribute('height', String(size));

  if (className) {
    const existingClasses = svgElement.getAttribute('class') || '';
    svgElement.setAttribute('class', existingClasses ? `${existingClasses} ${className}` : className);
  }

  return svgElement as SVGElement;
}
