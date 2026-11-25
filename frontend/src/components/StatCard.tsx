import type { ReactNode } from 'react';

interface StatCardProps {
  label: string;
  value: ReactNode;
  hint?: string;
  icon?: ReactNode;
  accent?: boolean;
}

export default function StatCard({ label, value, hint, icon, accent }: StatCardProps) {
  return (
    <div className={`panel p-4 ${accent ? 'ring-1 ring-emerald-500/30' : ''}`}>
      <div className="flex items-start justify-between gap-2">
        <div className="stat-label">{label}</div>
        {icon && <span className="text-emerald-400/70">{icon}</span>}
      </div>
      <div className={`mt-2 font-bold tabular-nums ${accent ? 'text-3xl text-emerald-300' : 'text-2xl text-white'}`}>
        {value}
      </div>
      {hint && <div className="mt-1 text-[11px] text-slate-500">{hint}</div>}
    </div>
  );
}
