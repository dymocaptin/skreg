import { useState, useEffect, useCallback, useRef } from 'react'
import { searchPackages } from '../../api.js'
import PackageCard from '../PackageCard/PackageCard.jsx'
import styles from './PackageGrid.module.css'

export default function PackageGrid({ query, category }) {
  const [packages, setPackages] = useState([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  const fetchSeqRef = useRef(0)

  const fetchPage = useCallback(async (q, cat, pg, append = false) => {
    const seq = ++fetchSeqRef.current
    setLoading(true)
    setError(null)
    try {
      const data = await searchPackages({ query: q, category: cat, page: pg })
      if (seq !== fetchSeqRef.current) return // discard stale response
      setPackages(prev => append ? [...prev, ...data.packages] : data.packages)
      setTotal(data.total)
    } catch (err) {
      if (seq !== fetchSeqRef.current) return
      setError(err.message)
    } finally {
      if (seq === fetchSeqRef.current) setLoading(false)
    }
  }, [])

  useEffect(() => {
    setPage(1)
    fetchPage(query, category, 1, false)
  }, [query, category, fetchPage])

  function handleLoadMore() {
    const next = page + 1
    setPage(next)
    fetchPage(query, category, next, true)
  }

  if (error) {
    return <p className={styles.message}>Failed to load packages: {error}</p>
  }

  return (
    <section className={styles.section}>
      {loading && packages.length === 0 ? (
        <p className={styles.message}>Loading…</p>
      ) : (
        <>
          <table className={styles.table}>
            <thead>
              <tr>
                <th className={styles.th}>NAME</th>
                <th className={styles.th}>NAMESPACE</th>
                <th className={styles.th}>VERSION</th>
                <th className={styles.th}>DESCRIPTION</th>
                <th className={styles.th}></th>
              </tr>
            </thead>
            <tbody>
              {packages.map(pkg => <PackageCard key={pkg.id} pkg={pkg} />)}
            </tbody>
          </table>
          {packages.length < total && (
            <button
              className={styles.loadMore}
              onClick={handleLoadMore}
              disabled={loading}
            >
              {loading ? 'Loading…' : 'Load more'}
            </button>
          )}
        </>
      )}
    </section>
  )
}
