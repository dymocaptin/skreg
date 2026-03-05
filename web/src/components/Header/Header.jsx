import styles from './Header.module.css'

export default function Header({ theme, onThemeToggle }) {
  return (
    <header className={styles.header}>
      <div className={styles.left}>
        <span className={styles.logo}>skreg</span>
        <span className={styles.context}>[skreg.ai]</span>
        <span className={styles.breadcrumb}>▸ Packages</span>
      </div>
      <div className={styles.right}>
        <button
          className={styles.themeToggle}
          onClick={onThemeToggle}
          aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          aria-pressed={theme === 'dark'}
        >
          <span aria-hidden="true">{theme === 'dark' ? '☀️' : '🌙'}</span>
        </button>
        <span className={styles.help}>?:help</span>
      </div>
    </header>
  )
}
