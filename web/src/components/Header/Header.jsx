import styles from './Header.module.css'

export default function Header({ query, onQueryChange, theme, onThemeToggle }) {
  return (
    <header className={styles.header}>
      <span className={styles.logo}>skreg</span>
      <input
        className={styles.search}
        type="search"
        role="searchbox"
        placeholder="Search packages…"
        value={query}
        onChange={e => onQueryChange(e.target.value)}
        aria-label="Search packages"
      />
      <button
        className={styles.themeToggle}
        onClick={onThemeToggle}
        aria-label="Toggle theme"
      >
        {theme === 'dark' ? '☀️' : '🌙'}
      </button>
    </header>
  )
}
