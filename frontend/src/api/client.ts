import axios, { AxiosInstance, AxiosResponse, InternalAxiosRequestConfig } from 'axios'
import type { ApiError } from '@/types/common'

// Create axios instance with default config.
//
// baseURL resolves through VITE_API_URL (default '/v1') so the frontend and
// auth-context share a single source of truth. Dev: relative '/v1' is forwarded
// by the vite proxy to the backend. Prod: same-origin '/v1' works with the
// Phase 21 single-container deploy; override VITE_API_URL with an absolute URL
// (e.g. https://api.rrules.example.com/v1) for a separate API host (D-22-05).
const apiClient: AxiosInstance = axios.create({
  baseURL: import.meta.env.VITE_API_URL || '/v1',
  headers: {
    'Content-Type': 'application/json',
  },
})

// Request interceptor - add auth token
apiClient.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const token = localStorage.getItem('auth_token')
    if (token) {
      config.headers.Authorization = `Bearer ${token}`
    }
    return config
  },
  (error) => {
    return Promise.reject(error)
  }
)

// Response interceptor - handle errors
apiClient.interceptors.response.use(
  (response: AxiosResponse) => response,
  (error) => {
    const status = error.response?.status

    if (status === 401) {
      // Token expired or invalid: drop it and tell the app, but do NOT navigate.
      //
      // A hard `window.location.href = '/login'` here tore down the running SPA
      // on every unauthorised response — including the several that fire in
      // parallel when a page loads — which left the browser stuck mid-navigation
      // and the page blank. Clearing the session and announcing it lets the
      // router render the page (with its own error state) and the navbar offer
      // the login link.
      localStorage.removeItem('auth_token')
      localStorage.removeItem('user')
      window.dispatchEvent(new CustomEvent('auth:unauthorized'))
    }

    return Promise.reject(error)
  }
)

// Error handler helper
export function handleError(error: unknown): never {
  if (axios.isAxiosError(error)) {
    const apiError: ApiError = error.response?.data || {
      code: 'NETWORK_ERROR',
      message: error.message,
    }
    throw apiError
  }
  throw { code: 'UNKNOWN_ERROR', message: String(error) }
}

export default apiClient
