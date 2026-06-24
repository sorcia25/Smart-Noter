import styles from './SearchSnippet.module.css';

const START = '\u{2068}';
const END = '\u{2069}';

/** Render an FTS snippet, converting the ⁨…⁩ marker pairs to <mark>. */
export function SearchSnippet({ text }: { text: string }) {
  const parts = text.split(START).flatMap((chunk, i) => {
    if (i === 0) return [{ hit: false, s: chunk }];
    const [hit = '', ...rest] = chunk.split(END);
    return [
      { hit: true, s: hit },
      { hit: false, s: rest.join(END) },
    ];
  });
  // Stable keys from each fragment's running character offset (fragments never
  // reorder within a snippet, and each begins at a distinct offset).
  let offset = 0;
  const keyed = parts.map((p) => {
    const part = { ...p, key: `${offset}:${p.hit ? 'h' : 'n'}` };
    offset += p.s.length;
    return part;
  });
  return (
    <span className={styles.snippet}>
      {keyed.map((p) =>
        p.hit ? (
          <mark key={p.key} className={styles.mark}>
            {p.s}
          </mark>
        ) : (
          <span key={p.key}>{p.s}</span>
        )
      )}
    </span>
  );
}
