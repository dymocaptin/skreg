import { useState, useEffect, useRef } from 'react'
import { previewPackage } from '../../api.js'
import styles from './PackageDetail.module.css'

export default function PackageDetail({ pkg }) {
  const [preview, setPreview] = useState({ status: 'loading' })
  const [copied, setCopied] = useState(false)
  const timerRef = useRef(null)
  const installCmd = `skreg install ${pkg.namespace}/${pkg.name}`

  // Reset copied state when package identity changes
  useEffect(() => {
    setCopied(false)
  }, [pkg.namespace, pkg.name])

  // Fetch preview data
  useEffect(() => {
    setPreview({ status: 'loading' })
    if (!pkg.latest_version) {
      setPreview({ status: 'failed', message: 'No version available' })
      return
    }
    const controller = new AbortController()
    previewPackage(pkg.namespace, pkg.name, pkg.latest_version, controller.signal)
      .then(data => setPreview({ status: 'loaded', ...data }))
      .catch(err => {
        if (err.name === 'AbortError') return
        setPreview({ status: 'failed', message: err.message })
      })
    return () => controller.abort()
  }, [pkg.namespace, pkg.name, pkg.latest_version])

  // Cleanup copy timer on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

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
        <span className={styles.pkgName}>{pkg.name}</span>
        {pkg.verification === 'publisher' && (
          <span className={styles.trustedBadge}>✓ trusted</span>
        )}
      </div>
      <div className={styles.panes}>
        <div className={styles.versions}>
          <div className={styles.version}>▶ {pkg.latest_version ?? 'unknown'}</div>
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
        <div className={styles.files}>
          {preview.status === 'loading' && (
            <p className={styles.loading}>⠙ Loading…</p>
          )}
          {preview.status === 'failed' && (
            <p className={styles.error}>{preview.message}</p>
          )}
          {preview.status === 'loaded' && (
            <>
              <span className={styles.fileRoot}>
                {pkg.name}@{pkg.latest_version ?? 'unknown'}/
              </span>
              {preview.files.map(path => (
                <span key={path} className={styles.fileEntry}>{path}</span>
              ))}
            </>
          )}
        </div>
        <div className={styles.skillmd}>
          {preview.status === 'loading' && (
            <p className={styles.loading}>⠙ Loading…</p>
          )}
          {preview.status === 'failed' && (
            <p className={styles.error}>{preview.message}</p>
          )}
          {preview.status === 'loaded' && (
            <>
              <pre className={styles.pre}>{preview.skill_md}</pre>
              {preview.truncated && (
                <p className={styles.truncated}>[truncated]</p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  )
}
