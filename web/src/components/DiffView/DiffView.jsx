import styles from './DiffView.module.css'

const LINE_CLASS = {
  context: styles.context,
  insert: styles.insert,
  delete: styles.delete,
}

const LINE_PREFIX = { context: ' ', insert: '+', delete: '-' }

export default function DiffView({ diff }) {
  if (!diff.files.length) {
    return <p className={styles.empty}>No changes between these versions.</p>
  }
  return (
    <div className={styles.diff}>
      {diff.files.map(file => (
        <div key={file.path} className={styles.file}>
          <div className={styles.fileHeader}>
            <span className={styles.filePath}>{file.path}</span>
            <span className={`${styles.badge} ${styles[file.status]}`}>{file.status}</span>
          </div>
          {file.binary ? (
            <p className={styles.binary}>Binary file differs</p>
          ) : (
            file.hunks.map((hunk, hi) => (
              <div key={hi} className={styles.hunk}>
                <div className={styles.hunkHeader}>
                  @@ -{hunk.old_start},{hunk.old_lines} +{hunk.new_start},{hunk.new_lines} @@
                </div>
                {hunk.lines.map((line, li) => (
                  <pre key={li} className={`${styles.line} ${LINE_CLASS[line.kind]}`}>
                    {LINE_PREFIX[line.kind]}{line.text}
                  </pre>
                ))}
              </div>
            ))
          )}
        </div>
      ))}
    </div>
  )
}
