import { Icon } from '@/components/primitives/Icon/Icon';
import styles from './SearchBox.module.css';

export interface SearchBoxProps {
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
  className?: string;
}

export function SearchBox({ value, onChange, placeholder, className }: SearchBoxProps) {
  return (
    <div className={[styles.box, className].filter(Boolean).join(' ')}>
      <Icon name="search" size={14} className={styles.icon} />
      <input
        type="search"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
