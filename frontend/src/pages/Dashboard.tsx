import {
  Activity, Ban, Clock3, Coins, Database, EyeOff, Fingerprint, Gauge,
  KeyRound, LockKeyhole, Play, Radio, ShieldAlert, ShieldCheck, Square,
  Timer, Wallet, Zap,
} from 'lucide-react';
import type { ReactNode } from 'react';
import StatCard from '../components/StatCard';
import { formatCoinAmount, type AppConfig, type HitRecord, type LogEvent, type ScanStats } from '../lib/tauri';

interface Props {
  stats: ScanStats;
  config: AppConfig;
  events: LogEvent[];
  history: number[];
  hits: HitRecord[];
  onStart: () => void;
  onStop: () => void;
  onHide: () => void;
}

const fmt = (n: number) => new Intl.NumberFormat('ru-RU').format(Math.round(n));
const uptime = (s: number) => {
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}ч ${m}м`;
};

function Sparkline({ data }: { data: number[] }) {
  const points = data.length > 1
    ? data.map((v, i) => `${(i / (data.length - 1)) * 100},${40 - Math.min(38, (v / Math.max(1, ...data)) * 38)}`).join(' ')
    : '0,38 100,38';
  return (
    <svg viewBox="0 0 100 40" preserveAspectRatio="none" className="h-24 w-full">
      <defs>
        <linearGradient id="spark" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#22d39a" stopOpacity="0.45" />
          <stop offset="100%" stopColor="#22d39a" stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon points={`0,40 ${points} 100,40`} fill="url(#spark)" />
      <polyline points={points} fill="none" stroke="#22d39a" strokeWidth="1.4" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Shield({ on, icon, label }: { on: boolean; icon: ReactNode; label: string }) {
  return (
    <div className={`flex items-center gap-2 rounded-lg border px-2.5 py-2 text-[11px] ${
      on ? 'border-emerald-500/25 bg-emerald-500/10 text-emerald-300' : 'border-slate-700/60 bg-slate-900/40 text-slate-500'
    }`}>
      {icon} <span className="truncate2">{label}</span>
    </div>
  );
}

export default function Dashboard({ stats, config, events, history, hits, onStart, onStop, onHide }: Props) {
  const full = config.full_anonymity_mode;
  const shields = [
    { on: config.anonymity_mode, label: 'Анонимный режим', icon: <ShieldCheck size={14} /> },
    { on: config.use_tor || !!config.proxy_url, label: 'Tor / прокси', icon: <Fingerprint size={14} /> },
    { on: config.random_user_agent && config.random_headers, label: 'Рандом заголовки', icon: <EyeOff size={14} /> },
    { on: config.encryption_enabled, label: 'AES-256-GCM', icon: <LockKeyhole size={14} /> },
    { on: config.memory_only_mode, label: 'Только в памяти', icon: <Database size={14} /> },
    { on: config.kill_switch, label: 'Килл-свитч', icon: <ShieldAlert size={14} /> },
    { on: config.block_direct_connection, label: 'Блок direct HTTP', icon: <Ban size={14} /> },
    { on: config.stealth_window_title, label: 'Маска окна', icon: <EyeOff size={14} /> },
  ];

  return (
    <div className="grid-bg min-h-full space-y-5 p-6">
      <section className="hero-glow relative overflow-hidden rounded-2xl border border-emerald-500/15 bg-[#0d1a22]/70 p-7">
        <div className="absolute -right-10 -top-16 h-56 w-56 rounded-full bg-emerald-500/10 blur-3xl" />
        <div className="relative flex flex-wrap items-center justify-between gap-6">
          <div>
            <div className="mb-2 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-emerald-400/80">
              <Radio size={13} /> Fully autonomous search
            </div>
            <h1 className="text-3xl font-bold text-white">Одна кнопка — и всё работает</h1>
            <p className="mt-2 max-w-xl text-sm leading-relaxed text-slate-400">
              14 монет · 15 режимов · бесконечный фоновый поиск без повторов.
              Анонимность, шифрование и защита от утечек включаются автоматически.
            </p>
          </div>
          <div className="flex items-center gap-3">
            {stats.running ? (
              <button className="btn-danger" onClick={onStop}><Square size={17} /> Остановить</button>
            ) : (
              <button className="btn-primary !px-7 !py-3.5 !text-base" onClick={onStart}><Play size={19} fill="currentColor" /> СТАРТ АВТОНОМНОГО ПОИСКА</button>
            )}
            <button className="btn-ghost" onClick={onHide}><EyeOff size={16} /> В трей</button>
          </div>
        </div>
        <div className="relative mt-5 flex flex-wrap gap-2">
          <span className="chip border-emerald-500/25 bg-emerald-500/10 text-emerald-300"><Zap size={12} /> {config.workers} воркеров</span>
          <span className="chip border-slate-600/60 text-slate-300"><Gauge size={12} /> батч {config.batch_size}</span>
          <span className="chip border-slate-600/60 text-slate-300"><Timer size={12} /> задержка {config.delay_ms} мс</span>
          <span className="chip border-slate-600/60 text-slate-300"><Activity size={12} /> {config.scan_mode}</span>
          {full && <span className="chip border-emerald-500/25 bg-emerald-500/10 text-emerald-300"><ShieldCheck size={12} /> полная анонимность</span>}
        </div>
      </section>

      <div className="grid grid-cols-2 gap-4 md:grid-cols-3 xl:grid-cols-6">
        <StatCard label="Проверено" value={fmt(stats.total_checked)} icon={<Activity size={16} />} accent />
        <StatCard label="Находки" value={fmt(stats.hits)} hint="кошельки с балансом" icon={<Coins size={16} />} />
        <StatCard label="Скорость" value={`${fmt(stats.checks_per_hour)}/ч`} icon={<Gauge size={16} />} />
        <StatCard label="В работе" value={uptime(stats.uptime_seconds)} icon={<Clock3 size={16} />} />
        <StatCard label="В кэше" value={fmt(stats.cache_size)} hint="адреса с TTL" icon={<Database size={16} />} />
        <StatCard label="Ошибки API" value={fmt(stats.errors)} hint={stats.kill_switch_active ? 'килл-свитч активен' : 'автопереключение API'} icon={<Ban size={16} />} />
      </div>

      <div className="grid gap-5 xl:grid-cols-3">
        <div className="panel p-5 xl:col-span-2">
          <div className="panel-title"><Activity size={16} /> Динамика проверок</div>
          <Sparkline data={history} />
          <div className="mt-2 grid grid-cols-3 gap-3 text-center text-[11px] text-slate-500">
            <div>пусто: <span className="text-slate-300">{fmt(stats.empty)}</span></div>
            <div>были поступления: <span className="text-slate-300">{fmt(stats.received_but_empty)}</span></div>
            <div>уникальных: <span className="text-slate-300">{fmt(stats.seen_size)}</span></div>
          </div>
        </div>
        <div className="panel p-5">
          <div className="panel-title"><ShieldCheck size={16} /> Защита включена</div>
          <div className="grid grid-cols-2 gap-2">{shields.map((s) => <Shield key={s.label} {...s} />)}</div>
        </div>
      </div>

      <div className="grid gap-5 xl:grid-cols-2">
        <div className="panel p-5">
          <div className="panel-title"><Radio size={16} /> Живой лог</div>
          <div className="h-56 space-y-1.5 overflow-y-auto pr-1 font-mono text-[11px]">
            {events.length === 0 && <div className="text-slate-600">События появятся после старта…</div>}
            {events.map((e, i) => (
              <div key={`${e.time}-${i}`} className={`flex gap-2 ${e.kind === 'hit' ? 'text-emerald-300' : e.kind === 'error' ? 'text-rose-300' : 'text-slate-400'}`}>
                <span className="text-slate-600">{e.time}</span><span className="truncate2">{e.text}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="panel p-5">
          <div className="panel-title"><Wallet size={16} /> Последняя проверка</div>
          <dl className="space-y-3 text-xs">
            <div className="flex justify-between gap-4"><dt className="text-slate-500">Монета</dt><dd className="font-semibold text-emerald-300">{stats.last_coin}</dd></div>
            <div className="flex justify-between gap-4"><dt className="text-slate-500">Адрес</dt><dd className="mono truncate2 max-w-[65%] text-slate-200">{stats.last_address}</dd></div>
            <div className="flex justify-between gap-4"><dt className="text-slate-500">Фраза / ключ</dt><dd className="mono truncate2 max-w-[65%] text-slate-200">{stats.last_phrase}</dd></div>
            <div className="flex justify-between gap-4"><dt className="text-slate-500">Баланс</dt><dd className="text-slate-200">{formatCoinAmount(stats.last_coin, stats.last_balance)}</dd></div>
          </dl>
          <div className="mt-4 border-t border-slate-700/50 pt-3 text-[11px] text-slate-500">
            Последняя находка: {hits[0] ? `${hits[0].coin} · ${formatCoinAmount(hits[0].coin, hits[0].balance)}` : 'пока нет'}
          </div>
        </div>
      </div>
    </div>
  );
}
