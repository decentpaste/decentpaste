// DOM utility functions

export function $(selector: string): HTMLElement | null {
  return document.querySelector(selector);
}

export function $$(selector: string): NodeListOf<HTMLElement> {
  return document.querySelectorAll(selector);
}

export function createElement<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props?: Partial<HTMLElementTagNameMap[K]> & { className?: string },
  children?: (HTMLElement | string)[],
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);

  if (props) {
    const { className, ...rest } = props;
    if (className) {
      element.className = className;
    }
    Object.assign(element, rest);
  }

  if (children) {
    children.forEach((child) => {
      if (typeof child === 'string') {
        element.appendChild(document.createTextNode(child));
      } else {
        element.appendChild(child);
      }
    });
  }

  return element;
}

export function html(strings: TemplateStringsArray, ...values: any[]): string {
  return strings.reduce((result, string, i) => {
    const value = values[i] ?? '';
    return result + string + (typeof value === 'string' ? escapeHtml(value) : value);
  }, '');
}

export function escapeHtml(str: string): string {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

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

export function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str;
  return str.slice(0, maxLength) + '...';
}

export function getStatusColor(status: string): string {
  switch (status) {
    case 'Connected':
      return 'bg-green-500';
    case 'Connecting':
      return 'bg-yellow-500';
    case 'Disconnected':
      return 'bg-gray-400';
    default:
      return 'bg-red-500';
  }
}

export function getStatusText(status: any): string {
  if (typeof status === 'string') {
    return status;
  }
  if (status && typeof status === 'object' && 'Error' in status) {
    return `Error: ${status.Error}`;
  }
  return 'Unknown';
}
