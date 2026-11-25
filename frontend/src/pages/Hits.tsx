import { useMemo, useState } from 'react';
import { Coins, Download, Search, Trash2, Wallet } from 'lucide-react';
import type { HitRecord } from '../lib/tauri';

interface Props {
  hits: HitRecord[];
  onClear: () => void;
}

const fmt = (n: number) => new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 8 }).format(n);

function download(name: string, text: string, mime: string) {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export default function Hits({ hits, onClear }: Props) {
  const [query, setQuery] = useState('');
  const [coin, setCoin] = useState('ALL');

  const coins = useMemo(() => Array.from(new Set(hits.map((h) => h.coin))).sort(), [hits]);
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return hits.filter((h) => {
      if (coin !== 'ALL' && h.coin !== coin) return false;
      if (!q) return true;
      return [h.address, h.phrase, h.coin, h.source].some((field) => field.toLowerCase().includes(q));
    });
  }, [hits, query, coin]);

  const total = filtered.reduce((sum, h) => sum + h.balance, 0);

  const exportCsv = () => {
    const rows = [
      ['time', 'coin', 'address', 'phrase', 'source', 'balance', 'received'],
      ...filtered.map((h) => [h.time, h.coin, h.address, h.phrase, h.source, String(h.balance), String(h.received)]),
    ];
    download('happiness-hits.csv', rows.map((row) => row.map((cell) => `"${cell.replace(/"/g, '""')}"`).join(',')).join('\n'), 'text/csv');
  };

  return (
    <div className="grid-bg min-h-full space-y-5 p-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">Найденные кошельки</h1>
          <p className="mt-1 text-xs text-slate-500">{fmt(hits.length)} находок · суммарный баланс: {fmt(total)} · все данные остаются локально</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button className="btn-ghost" onClick={exportCsv}><Download size={15} /> CSV</button>
          <button className="btn-ghost" onClick={() => download('happiness-hits.json', JSON.stringify(filtered, null, 2), 'application/json')}>
            <Download size={15} /> JSON
          </button>
          <button className="btn-danger" onClick={onClear}><Trash2 size={15} /> Очистить</button>
        </div>
      </div>

      <div className="panel p-4">
        <div className="flex flex-wrap gap-3">
          <div className="relative min-w-[240px] flex-1">
            <Search size={15} className="absolute left-3 top-3 text-slate-500" />
            <input className="input pl-9" placeholder="Поиск по адресу, фразе, монете…" value={query}
              onChange={(e) => setQuery(e.target.value)} />
          </div>
          <select className="input max-w-[180px]" value={coin} onChange={(e) => setCoin(e.target.value)}>
            <option value="ALL">Все монеты</option>
            {coins.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
        </div>
      </div>

      <div className="panel overflow-hidden">
        {filtered.length === 0 ? (
          <div className="flex flex-col items-center gap-3 px-6 py-16 text-center text-slate-500">
            <Wallet size={36} className="text-slate-600" />
            <div className="text-sm">Находок пока нет. Сканер работает бесконечно — шанс растёт с каждой проверкой.</div>
            <div className="text-[11px] text-slate-600">Это статистический поиск: шанс найти заполненный кошелёк очень мал.</div>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead className="bg-slate-900/70 text-[10px] uppercase tracking-wider text-slate-500">
                <tr>
                  <th className="px-4 py-3">Время</th><th className="px-4 py-3">Монета</th>
                  <th className="px-4 py-3">Адрес</th><th className="px-4 py-3">Фраза / ключ</th>
                  <th className="px-4 py-3">Режим</th><th className="px-4 py-3 text-right">Баланс</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/70">
                {filtered.map((hit) => (
                  <tr key={hit.id} className="transition hover:bg-emerald-500/5">
                    <td className="whitespace-nowrap px-4 py-3 text-slate-500">{hit.time}</td>
                    <td className="px-4 py-3"><span className="chip border-emerald-500/20 bg-emerald-500/10 text-emerald-300"><Coins size={11} />{hit.coin}</span></td>
                    <td className="mono max-w-[220px] truncate px-4 py-3 text-slate-200" title={hit.address}>{hit.address}</td>
                    <td className="mono max-w-[260px] truncate px-4 py-3 text-slate-400" title={hit.phrase}>{hit.phrase}</td>
                    <td className="px-4 py-3 text-slate-500">{hit.source}</td>
                    <td className="whitespace-nowrap px-4 py-3 text-right font-semibold text-emerald-300">{fmt(hit.balance)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
