import { useEffect, useRef } from 'react';
import { Check, Minus } from 'lucide-react';
import { cn } from '../../utils/cn';

interface AccountSelectionProps {
    checked: boolean;
    indeterminate?: boolean;
    disabled?: boolean;
    label: string;
    onChange: () => void;
}

export default function AccountSelection({ checked, indeterminate = false, disabled = false, label, onChange }: AccountSelectionProps) {
    const inputRef = useRef<HTMLInputElement>(null);
    useEffect(() => {
        if (inputRef.current) inputRef.current.indeterminate = indeterminate;
    }, [indeterminate]);

    return (
        <label className="relative flex items-center justify-center w-6 h-6 shrink-0" title={label}>
            <input
                ref={inputRef}
                type="checkbox"
                aria-label={label}
                checked={checked}
                disabled={disabled}
                onChange={onChange}
                onClick={event => event.stopPropagation()}
                className="peer absolute inset-0 w-full h-full opacity-0 cursor-pointer disabled:cursor-not-allowed"
            />
            <span className={cn(
                "pointer-events-none flex items-center justify-center w-[18px] h-[18px] rounded-full border transition-all peer-focus-visible:ring-2 peer-focus-visible:ring-blue-500/50 peer-focus-visible:ring-offset-2 dark:peer-focus-visible:ring-offset-base-100",
                checked || indeterminate
                    ? "border-blue-500 bg-blue-500 text-white shadow-sm shadow-blue-500/20 dark:border-blue-400 dark:bg-blue-400 dark:text-blue-950"
                    : "border-gray-300 dark:border-gray-600 peer-hover:border-blue-400 peer-hover:bg-blue-50 dark:peer-hover:bg-blue-900/20",
                disabled && "opacity-40"
            )}>
                {indeterminate ? <Minus className="w-3 h-3" strokeWidth={2.5} /> : checked && <Check className="w-3 h-3" strokeWidth={2.5} />}
            </span>
        </label>
    );
}
