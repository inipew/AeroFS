import axios from 'axios';

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/api/v1',
  withCredentials: true,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Safe UUID generator — crypto.randomUUID() requires Android 12+ / Chrome 92+.
// Falls back to Math.random()-based UUID v4 on older WebViews.
function generateUUID(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

// Auto-inject X-Request-ID on all outgoing HTTP requests (Plan 40 P0.8)
apiClient.interceptors.request.use((config) => {
  if (!config.headers['x-request-id']) {
    config.headers['x-request-id'] = generateUUID();
  }
  return config;
});

export default apiClient;
