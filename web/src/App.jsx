import { useState, useEffect } from 'react'
import Header from './components/Header/Header.jsx'
import Subheader from './components/Subheader/Subheader.jsx'
import PackageGrid from './components/PackageGrid/PackageGrid.jsx'
import styles from './App.module.css'

export default function App() {
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') ?? 'dark')

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : '')
    localStorage.setItem('theme', theme)
  }, [theme])

  function handleThemeToggle() {
    setTheme(t => t === 'dark' ? 'light' : 'dark')
  }

  return (
    <div className={styles.app}>
      <Header theme={theme} onThemeToggle={handleThemeToggle} />
      <Subheader />
      <PackageGrid />
    </div>
  )
}
