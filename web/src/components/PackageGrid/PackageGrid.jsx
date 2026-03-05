import { useState, useEffect, useCallback } from 'react'
import { searchPackages } from '../../api.js'
import PackageCard from '../PackageCard/PackageCard.jsx'
import styles from './PackageGrid.module.css'

export default function PackageGrid({ query, category }) {
  const [packages, setPackages] = useState([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  const fetchPage = useCallback(async (q, cat, pg, append = false) => {
    setLoading(true)
    setError(null)
    try {
      const data = await searchPackages({ query: q, category: cat, page: pg })
      setPackages(prev => append ? [...prev, ...data.packages] : data.packages)
      setTotal(data.total)
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
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
          <div className={styles.grid}>
            {packages.map(pkg => <PackageCard key={pkg.id} pkg={pkg} />)}
          </div>
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
