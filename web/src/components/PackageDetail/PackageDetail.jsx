import { useState, useRef, useEffect } from 'react'
import styles from './PackageDetail.module.css'

export default function PackageDetail({ pkg }) {
  const [copied, setCopied] = useState(false)
  const installCmd = `skreg install ${pkg.namespace}/${pkg.name}`
  const timerRef = useRef(null)

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  useEffect(() => {
    setCopied(false)
  }, [pkg])

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(installCmd)
      setCopied(true)
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => setCopied(false), 1500)
    } catch {
      // clipboard write failed — silently ignore
    }
  }

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <span className={styles.name}>{pkg.name}</span>
        <span className={styles.ref}>{pkg.namespace}/{pkg.name}@{pkg.latest_version}</span>
      </div>
      {pkg.category && (
        <span className={styles.category}>{pkg.category}</span>
      )}
      {pkg.verification && (
        <span className={pkg.verification === 'publisher'
          ? styles.badgePublisher
          : styles.badgeSelf}>
          {pkg.verification === 'publisher' ? '✦ publisher verified' : '◈ self-signed'}
        </span>
      )}
      <p className={styles.description}>{pkg.description}</p>
      <div className={styles.installBlock}>
        <code className={styles.installCmd}>$ {installCmd}</code>
        <button
          className={styles.copyBtn}
          onClick={handleCopy}
          aria-label="Copy install command"
        >
          {copied ? 'copied!' : 'copy'}
        </button>
      </div>
    </div>
  )
}
