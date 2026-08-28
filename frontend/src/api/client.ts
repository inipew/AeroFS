import axios from 'axios';

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/api/v1',
  withCredentials: true,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Auto-inject X-Request-ID on all outgoing HTTP requests (Plan 40 P0.8)
apiClient.interceptors.request.use((config) => {
  if (!config.headers['x-request-id']) {
    config.headers['x-request-id'] = crypto.randomUUID();
  }
  return config;
});

export default apiClient;
