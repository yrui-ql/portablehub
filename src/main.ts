import { mount } from 'svelte'
import { invoke } from '@tauri-apps/api/core'
import './app.css'
import App from './App.svelte'

function log(message: string) {
  invoke('log_frontend', { message }).catch(() => {
    /* logging must never break the app */
  })
}

window.addEventListener('error', (e) => {
  log(`error: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`)
})
window.addEventListener('unhandledrejection', (e) => {
  log(`rejection: ${String(e.reason)}`)
})

const target = document.getElementById('app')
if (!target) throw new Error('#app not found')

let app
try {
  app = mount(App, { target })
} catch (e) {
  log(`mount failed: ${String(e)}`)
  throw e
}

export default app
