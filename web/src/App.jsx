import { useState, useEffect } from 'react'
import Header from './components/Header/Header.jsx'
import CategoryFilter from './components/CategoryFilter/CategoryFilter.jsx'
import PackageGrid from './components/PackageGrid/PackageGrid.jsx'
import styles from './App.module.css'

const CATEGORIES = ['agents', 'tools', 'formatters', 'analyzers', 'writers']

export default function App() {
  const [theme, setTheme] = useState(() => localStorage.getItem('theme') ?? 'dark')
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : '')
    localStorage.setItem('theme', theme)
  }, [theme])

  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), 300)
    return () => clearTimeout(id)
  }, [query])

  function handleThemeToggle() {
    setTheme(t => t === 'dark' ? 'light' : 'dark')
  }

  return (
    <div className={styles.app}>
      <Header
        query={query}
        onQueryChange={setQuery}
        theme={theme}
        onThemeToggle={handleThemeToggle}
      />
      <CategoryFilter
        categories={CATEGORIES}
        active={category}
        onChange={setCategory}
      />
      <PackageGrid query={debouncedQuery} category={category} />
    </div>
  )
}
