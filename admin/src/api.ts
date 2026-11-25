export type User = {
  id: string
  name: string
  surname: string
  email: string
  referral_code: string | null
  is_staff: boolean
  is_admin: boolean
  is_email_verified: boolean
}

type ApiError = { message?: string }

function getCookie(name: string): string | null {
  const match = document.cookie.match(new RegExp(`(?:^|; )${name}=([^;]*)`))
  return match ? decodeURIComponent(match[1]) : null
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const method = (init.method || 'GET').toUpperCase()
  const headers = new Headers(init.headers)
  if (init.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json')
  }
  if (['POST', 'PUT', 'PATCH', 'DELETE'].includes(method)) {
    const csrf = getCookie('csrf_token')
    if (csrf) headers.set('X-CSRF-Token', csrf)
  }

  const res = await fetch(path, {
    ...init,
    headers,
    credentials: 'include',
  })

  if (!res.ok) {
    let message = 'Request failed'
    try {
      const data = (await res.json()) as ApiError
      if (data.message) message = data.message
    } catch {
      // ignore
    }
    throw new Error(message)
  }

  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export async function login(email: string, password: string) {
  return request<{ message: string; user: User }>('/api/v1/auth/login/', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
}

export async function me() {
  return request<User>('/api/v1/auth/me/')
}

export async function logout() {
  return request<{ message: string }>('/api/v1/auth/logout/', { method: 'POST' })
}

export async function fetchUsers() {
  return request<{ users: User[] }>('/api/v1/admin/users/')
}

export async function updateUser(id: string, payload: { is_staff?: boolean; is_admin?: boolean }) {
  return request<User>(`/api/v1/admin/users/${id}/`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  })
}
