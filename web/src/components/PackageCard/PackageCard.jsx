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

  return (
    <tr className={styles.row}>
      <td className={styles.name}>{pkg.name}</td>
      <td className={styles.namespace}>{pkg.namespace}</td>
      <td className={styles.version}>v{pkg.latest_version}</td>
      <td className={styles.desc}>{pkg.description}</td>
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
