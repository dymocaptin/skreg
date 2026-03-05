import styles from './CategoryFilter.module.css'

export default function CategoryFilter({ categories, active, onChange }) {
  const all = [{ slug: '', label: 'All' }, ...categories.map(c => ({ slug: c, label: c }))]

  return (
    <nav className={styles.row} aria-label="Filter by category">
      {all.map(({ slug, label }) => (
        <button
          key={slug}
          className={`${styles.pill} ${active === slug ? styles.active : ''}`}
          onClick={() => onChange(slug)}
          aria-pressed={active === slug}
        >
          {label}
        </button>
      ))}
    </nav>
  )
}
