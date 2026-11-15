import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './styles/app.css'
import MenuBarApp from './MenuBarApp'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <MenuBarApp />
  </StrictMode>,
)
