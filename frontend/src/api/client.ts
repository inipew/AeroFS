import axios from 'axios';

export const apiClient = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/api/v1',
  withCredentials: true,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Attach Authorization header if session_id is saved in localStorage
apiClient.interceptors.request.use((config) => {
  const sessionId = localStorage.getItem('session_id');
  if (sessionId && !config.headers['Authorization']) {
    config.headers['Authorization'] = `Bearer ${sessionId}`;
  }
  return config;
});

// Auto clear session if 401 Unauthorized is returned
apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      localStorage.removeItem('session_id');
    }
    return Promise.reject(error);
  }
);
