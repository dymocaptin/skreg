import { useState } from 'react'
import styles from './PackageCard.module.css'

export default function PackageCard({ pkg }) {
  const [copied, setCopied] = useState(false)
  const installCmd = `skreg install ${pkg.namespace}/${pkg.name}`

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(installCmd)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // Clipboard write failed (e.g. permissions denied) — silently ignore
      // The button label stays as "$ copy" giving the user no false confirmation
    }
  }

  const date = new Date(pkg.created_at).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })

  return (
    <article className={styles.card}>
      <div className={styles.top}>
        <span className={styles.ref}>{pkg.namespace}/{pkg.name}</span>
        <span className={styles.badge}>{pkg.category}</span>
      </div>
      <p className={styles.desc}>{pkg.description}</p>
      <div className={styles.bottom}>
        <span className={styles.version}>v{pkg.latest_version}</span>
        <span className={styles.date}>{date}</span>
        <button
          className={styles.copy}
          onClick={handleCopy}
          aria-label="Copy install command"
        >
          {copied ? 'Copied!' : '$ copy'}
        </button>
      </div>
    </article>
  )
}
