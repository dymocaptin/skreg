import styles from './Header.module.css'

export default function Header({ query, onQueryChange, theme, onThemeToggle }) {
  return (
    <header className={styles.header}>
      <span className={styles.logo}>skreg</span>
      <search>
        <input
          className={styles.search}
          type="search"
          placeholder="Search packages…"
          value={query}
          onChange={e => onQueryChange(e.target.value)}
          aria-label="Search packages"
        />
      </search>
      <button
        className={styles.themeToggle}
        onClick={onThemeToggle}
        aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        aria-pressed={theme === 'dark'}
      >
        <span aria-hidden="true">{theme === 'dark' ? '☀️' : '🌙'}</span>
      </button>
    </header>
  )
}
