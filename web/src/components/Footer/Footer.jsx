import styles from './Footer.module.css'

export default function Footer({ searchOpen, query, resultCount }) {
  if (searchOpen) {
    return (
      <footer className={styles.footer}>
        <span className={styles.hint}><kbd className={styles.key}>Esc</kbd>clear</span>
        {query && (
          <span className={styles.filter}>
            Filter: "{query}" · {resultCount} result{resultCount !== 1 ? 's' : ''}
          </span>
        )}
      </footer>
    )
  }

  return (
    <footer className={styles.footer}>
      <span className={styles.hint}><kbd className={styles.key}>/</kbd>search</span>
      <span className={styles.hint}><kbd className={styles.key}>j</kbd><kbd className={styles.key}>k</kbd>navigate</span>
      <span className={styles.hint}><kbd className={styles.key}>g</kbd><kbd className={styles.key}>G</kbd>top/bottom</span>
      <span className={styles.hint}><kbd className={styles.key}>Enter</kbd>copy install</span>
      <span className={styles.hint}><kbd className={styles.key}>Esc</kbd>deselect</span>
    </footer>
  )
}
