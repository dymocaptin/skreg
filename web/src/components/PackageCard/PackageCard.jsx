import { useState } from 'react'
import styles from './PackageCard.module.css'

export default function PackageCard({ pkg, selected, onClick, hideDesc = false }) {
  const [copied, setCopied] = useState(false)
  const installCmd = `skreg install ${pkg.namespace}/${pkg.name}`

  async function handleCopy(e) {
    e.stopPropagation()
    try {
      await navigator.clipboard.writeText(installCmd)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      // Clipboard write failed (e.g. permissions denied) — silently ignore
    }
  }

  return (
    <tr
      className={`${styles.row} ${selected ? styles.selected : ''}`}
      onClick={onClick}
    >
      <td className={styles.name}>
        <span className={styles.cursor} aria-hidden="true">{selected ? '▶' : ' '}</span>
        {pkg.name}
      </td>
      <td className={styles.namespace}>{pkg.namespace}</td>
      <td className={styles.version}>v{pkg.latest_version}</td>
      {!hideDesc && <td className={styles.desc}>{pkg.description}</td>}
      <td className={styles.actions}>
        <button
          className={styles.copy}
          onClick={handleCopy}
          aria-label="Copy install command"
        >
          {copied ? 'Copied!' : '$ copy'}
        </button>
      </td>
    </tr>
  )
}
