import { useState, useEffect, useRef } from 'react'
import { previewPackage, listVersions, diffPackage } from '../../api.js'
import DiffView from '../DiffView/DiffView.jsx'
import styles from './PackageDetail.module.css'

export default function PackageDetail({ pkg }) {
  const [preview, setPreview] = useState({ status: 'loading' })
  const [copied, setCopied] = useState(false)
  const [versions, setVersions] = useState([])
  const [from, setFrom] = useState('')
  const [to, setTo] = useState('')
  const [diff, setDiff] = useState({ status: 'idle' })
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

  // Load the version list and default from/to to the latest two.
  useEffect(() => {
    const controller = new AbortController()
    setDiff({ status: 'idle' })
    listVersions(pkg.namespace, pkg.name, controller.signal)
      .then(data => {
        const vs = data.versions.map(v => v.version)
        setVersions(vs)
        setTo(vs[0] ?? '')
        setFrom(vs[1] ?? '')
      })
      .catch(err => {
        if (err.name !== 'AbortError') setVersions([])
      })
    return () => controller.abort()
  }, [pkg.namespace, pkg.name])

  const canCompare =
    versions.includes(from) && versions.includes(to) && from !== to

  function handleCompare() {
    if (!canCompare) return
    setDiff({ status: 'loading' })
    diffPackage(pkg.namespace, pkg.name, from, to)
      .then(data => setDiff({ status: 'loaded', data }))
      .catch(err => setDiff({ status: 'failed', message: err.message }))
  }

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
      <div className={styles.compare}>
        <div className={styles.compareControls}>
          <label>
            from
            <select value={from} onChange={e => setFrom(e.target.value)}>
              {versions.map(v => <option key={v} value={v}>{v}</option>)}
            </select>
          </label>
          <span className={styles.arrow}>→</span>
          <label>
            to
            <select value={to} onChange={e => setTo(e.target.value)}>
              {versions.map(v => <option key={v} value={v}>{v}</option>)}
            </select>
          </label>
          <button onClick={handleCompare} disabled={!canCompare}>Compare</button>
        </div>
        {diff.status === 'loading' && <p className={styles.loading}>⠙ Loading diff…</p>}
        {diff.status === 'failed' && <p className={styles.error}>{diff.message}</p>}
        {diff.status === 'loaded' && <DiffView diff={diff.data} />}
      </div>
    </div>
  )
}
