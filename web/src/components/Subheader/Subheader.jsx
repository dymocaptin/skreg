import styles from './Subheader.module.css'

export default function Subheader() {
  return (
    <div className={styles.bar}>
      <span className={styles.tagline}>
        A package registry for AI coding assistant skills — built on cryptographic publisher identity and trust.
      </span>
      <a
        className={styles.sourceLink}
        href="https://github.com/dymocaptin/skreg"
        target="_blank"
        rel="noopener noreferrer"
      >
        source
      </a>
    </div>
  )
}
