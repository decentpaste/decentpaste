// DOM utility functions

import type { NetworkStatus } from '../api/types';

/**
 * Shorthand for document.querySelector
 */
export function $(selector: string): HTMLElement | null {
  return document.querySelector(selector);
}

/**
 * Shorthand for document.querySelectorAll
 */
export function $$(selector: string): NodeListOf<HTMLElement> {
  return document.querySelectorAll(selector);
}

/**
 * Escapes HTML special characters to prevent XSS attacks.
 * Uses the browser's built-in text encoding via textContent.
 */
export function escapeHtml(str: string): string {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

/**
 * Formats an ISO timestamp into a human-readable relative time.
 * @param isoString - ISO 8601 timestamp string
 * @returns Human-readable string like "Just now", "5m ago", "2h ago", or a date
 */
export function formatTime(isoString: string): string {
  const date = new Date(isoString);
  const now = new Date();
  const diff = now.getTime() - date.getTime();

  if (diff < 60000) {
    return 'Just now';
  } else if (diff < 3600000) {
    const mins = Math.floor(diff / 60000);
    return `${mins}m ago`;
  } else if (diff < 86400000) {
    const hours = Math.floor(diff / 3600000);
    return `${hours}h ago`;
  } else {
    return date.toLocaleDateString();
  }
}

/**
 * Truncates a string to a maximum length, adding ellipsis if needed.
 */
export function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str;
  return str.slice(0, maxLength) + '...';
}

/**
 * Converts a NetworkStatus value to a human-readable string.
 * Handles both simple string statuses and object-based error states.
 */
export function getStatusText(status: NetworkStatus): string {
  if (typeof status === 'string') {
    return status;
  }
  if (status && typeof status === 'object' && 'Error' in status) {
    return `Error: ${status.Error}`;
  }
  return 'Unknown';
}
