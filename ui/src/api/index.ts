import { $fetch } from 'ofetch'

let loginPromise: Promise<void> | null = null

export async function loginAndRedirect(redirect?: string) {
  if (loginPromise) return loginPromise

  loginPromise = (async () => {
    try {
      const { loginUrl } = await $fetch<{ loginUrl: string }>('/api/auth/login', {
        method: 'POST',
        body: { redirect: redirect || window.location.pathname },
        ignoreResponseError: false,
      })
      window.location.href = loginUrl
    } catch {
      window.location.href = '/auth/error'
    }
  })()

  loginPromise.catch(() => {
    loginPromise = null
  })
}

export const apiFetch = $fetch.create({
  onResponseError({ response }) {
    if (response.status === 401) {
      loginAndRedirect()
    }
  },
})
