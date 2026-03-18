import { useState, useEffect, useCallback, useRef } from 'react'
import { searchPackages } from '../../api.js'
import PackageCard from '../PackageCard/PackageCard.jsx'
import PackageDetail from '../PackageDetail/PackageDetail.jsx'
import Footer from '../Footer/Footer.jsx'
import styles from './PackageGrid.module.css'

export default function PackageGrid() {
  const [packages, setPackages] = useState([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(null)

  const [selectedIndex, setSelectedIndex] = useState(-1)
  const [searchOpen, setSearchOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [debouncedQuery, setDebouncedQuery] = useState('')

  const tableRef = useRef(null)
  const searchInputRef = useRef(null)
  const fetchSeqRef = useRef(0)

  // Debounce search query
  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), 300)
    return () => clearTimeout(id)
  }, [query])

  const fetchPage = useCallback(async (q, pg, append = false) => {
    const seq = ++fetchSeqRef.current
    setLoading(true)
    setError(null)
    try {
      const data = await searchPackages({ query: q, page: pg })
      if (seq !== fetchSeqRef.current) return
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
    setSelectedIndex(-1)
    fetchPage(debouncedQuery, 1, false)
  }, [debouncedQuery, fetchPage])

  // Auto-focus search input when it opens
  useEffect(() => {
    if (searchOpen) searchInputRef.current?.focus()
  }, [searchOpen])

  // Scroll selected row into view
  useEffect(() => {
    if (selectedIndex < 0) return
    const rows = tableRef.current?.querySelectorAll('tbody tr')
    rows?.[selectedIndex]?.scrollIntoView?.({ block: 'nearest' })
  }, [selectedIndex])

  // Keyboard navigation
  useEffect(() => {
    function onKeyDown(e) {
      // Let the search input handle its own keys except Escape
      if (searchOpen && e.key !== 'Escape') return

      switch (e.key) {
        case '/':
          if (!searchOpen) {
            e.preventDefault()
            setSearchOpen(true)
          }
          break
        case 'Escape':
          if (searchOpen) {
            setSearchOpen(false)
            setQuery('')
          } else {
            setSelectedIndex(-1)
          }
          break
        case 'j':
        case 'ArrowDown':
          e.preventDefault()
          setSelectedIndex(i => Math.min(i + 1, packages.length - 1))
          break
        case 'k':
        case 'ArrowUp':
          e.preventDefault()
          setSelectedIndex(i => Math.max(i - 1, 0))
          break
        case 'g':
          if (!e.shiftKey) {
            e.preventDefault()
            setSelectedIndex(0)
          }
          break
        case 'G':
          e.preventDefault()
          setSelectedIndex(packages.length - 1)
          break
        case 'Enter':
          if (selectedIndex >= 0 && packages[selectedIndex]) {
            const pkg = packages[selectedIndex]
            navigator.clipboard.writeText(`skreg install ${pkg.namespace}/${pkg.name}`).catch(() => {})
          }
          break
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [searchOpen, packages, selectedIndex])

  function handleLoadMore() {
    const next = page + 1
    setPage(next)
    fetchPage(debouncedQuery, next, true)
  }

  return (
    <div className={styles.outer}>
      {searchOpen && (
        <div className={styles.searchBar}>
          <span className={styles.searchPrompt}>/</span>
          <input
            ref={searchInputRef}
            className={styles.searchInput}
            type="search"
            placeholder="search packages…"
            value={query}
            onChange={e => setQuery(e.target.value)}
            aria-label="Search packages"
          />
        </div>
      )}

      <div className={selectedIndex >= 0 ? styles.body : styles.scrollArea}>
        <div className={selectedIndex >= 0 ? styles.tablePane : undefined}>
          {error && <p className={styles.message}>Failed to load packages: {error}</p>}
          {loading && packages.length === 0 && <p className={styles.message}>Loading…</p>}
          {!error && (loading || packages.length > 0) && (
            <table ref={tableRef} className={styles.table}>
              <thead>
                <tr>
                  <th className={styles.th}>NAME</th>
                  <th className={styles.th}>NAMESPACE</th>
                  <th className={styles.th}>VERSION</th>
                  <th className={styles.th}>VERIFICATION</th>
                  {selectedIndex < 0 && <th className={styles.th}>DESCRIPTION</th>}
                  <th className={styles.th}></th>
                </tr>
              </thead>
              <tbody>
                {packages.map((pkg, i) => (
                  <PackageCard
                    key={pkg.id}
                    pkg={pkg}
                    selected={i === selectedIndex}
                    hideDesc={selectedIndex >= 0}
                    onClick={() => setSelectedIndex(i)}
                  />
                ))}
              </tbody>
            </table>
          )}
          {!loading && packages.length === 0 && !error && (
            <p className={styles.message}>
              {query ? `No packages found for "${query}"` : 'No packages found'}
            </p>
          )}
          {packages.length < total && (
            <button
              className={styles.loadMore}
              onClick={handleLoadMore}
              disabled={loading}
            >
              {loading ? 'Loading…' : 'Load more'}
            </button>
          )}
        </div>
        {selectedIndex >= 0 && packages[selectedIndex] && (
          <PackageDetail pkg={packages[selectedIndex]} />
        )}
      </div>

      <Footer searchOpen={searchOpen} query={query} resultCount={packages.length} />
    </div>
  )
}
