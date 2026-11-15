import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/app.css'
import { installDropGuard } from './lib/dropGuard'
import MenuBarApp from './MenuBarApp'

installDropGuard()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <MenuBarApp />
  </StrictMode>,
)
