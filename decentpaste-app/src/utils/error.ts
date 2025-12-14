// Error handling utilities

/**
 * Extracts a user-friendly error message from various error types.
 * Handles Error objects, strings, and unknown error types.
 */
export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object' && 'message' in error) {
    return String((error as { message: unknown }).message);
  }
  return 'An unexpected error occurred';
}

/**
 * Type guard to check if an error is an AbortError (e.g., from cancelled operations)
 */
export function isAbortError(error: unknown): boolean {
  return error instanceof Error && error.name === 'AbortError';
}

/**
 * Type guard to check if an error is a network-related error
 */
export function isNetworkError(error: unknown): boolean {
  if (error instanceof Error) {
    const message = error.message.toLowerCase();
    return (
      message.includes('network') ||
      message.includes('fetch') ||
      message.includes('connection') ||
      message.includes('timeout')
    );
  }
  return false;
}
