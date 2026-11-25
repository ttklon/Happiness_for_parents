/**
 * Typed bridge to the Rust/Tauri commands. When the UI is opened in a normal
 * browser (e.g. `npm run dev` for a live preview) a small in-memory simulator
 * is used so the interface can be explored without the desktop shell.
 */

export const COINS = ['BTC', 'ETH', 'LTC', 'DOGE', 'BCH', 'MATIC', 'BSC', 'ARB', 'OPT', 'BASE', 'AVAX', 'TRX', 'SOL', 'TON'];

export const MODES = [
  'ALL_COINS_AUTO', 'RANDOM', 'BRAIN_WALLET', 'BRAIN_COMBOS', 'MULTI_WORD_COMBO',
  'BIP39_PASSPHRASE', 'TARGET_ADDRESSES', 'MULTI_COIN_BRAIN', 'TRANSACTION_HISTORY',
  'BLOOM_FILTER', 'VANITY_PATTERN', 'SEQUENTIAL_KEYS', 'RICH_LIST', 'PUZZLE', 'DICTIONARY',
];

export interface AppConfig {
  workers: number;
  batch_size: number;
  delay_ms: number;
  coin: string;
  scan_mode: string;
  address_type: string;
  bip39_language: string;
  vanity_prefix: string;
  dictionary_file: string;
  save_empty: boolean;
  derive_all_paths: boolean;
  target_addresses_file: string;
  privacy_log_phrases: boolean;
  update_check_enabled: boolean;
  address_cache_enabled: boolean;
  address_cache_ttl_minutes: number;
  telegram_enabled: boolean;
  telegram_token: string;
  telegram_chat_id: string;
  discord_enabled: boolean;
  discord_webhook: string;
  full_anonymity_mode: boolean;
  anonymity_mode: boolean;
  use_tor: boolean;
  tor_proxy: string;
  proxy_url: string;
  proxy_list: string;
  proxy_rotation: boolean;
  jitter_min_ms: number;
  jitter_max_ms: number;
  random_user_agent: boolean;
  random_headers: boolean;
  stealth_mode: boolean;
  stealth_window_title: boolean;
  encryption_enabled: boolean;
  encryption_password: string;
  memory_only_mode: boolean;
  kill_switch: boolean;
  kill_switch_threshold: number;
  block_direct_connection: boolean;
  autostart: boolean;
  start_minimized: boolean;
}

export const DEFAULT_CONFIG: AppConfig = {
  workers: 2,
  batch_size: 25,
  delay_ms: 1200,
  coin: 'ALL',
  scan_mode: 'ALL_COINS_AUTO',
  address_type: 'auto',
  bip39_language: 'english',
  vanity_prefix: '',
  dictionary_file: '',
  save_empty: false,
  derive_all_paths: true,
  target_addresses_file: '',
  privacy_log_phrases: true,
  update_check_enabled: true,
  address_cache_enabled: true,
  address_cache_ttl_minutes: 60,
  telegram_enabled: false,
  telegram_token: '',
  telegram_chat_id: '',
  discord_enabled: false,
  discord_webhook: '',
  full_anonymity_mode: true,
  anonymity_mode: true,
  use_tor: false,
  tor_proxy: 'http://127.0.0.1:9050',
  proxy_url: '',
  proxy_list: '',
  proxy_rotation: false,
  jitter_min_ms: 200,
  jitter_max_ms: 1500,
  random_user_agent: true,
  random_headers: true,
  stealth_mode: true,
  stealth_window_title: true,
  encryption_enabled: false,
  encryption_password: '',
  memory_only_mode: false,
  kill_switch: true,
  kill_switch_threshold: 10,
  block_direct_connection: false,
  autostart: false,
  start_minimized: true,
};

export interface ScanStats {
  running: boolean;
  workers: number;
  total_checked: number;
  hits: number;
  empty: number;
  received_but_empty: number;
  cache_size: number;
  seen_size: number;
  checks_per_hour: number;
  uptime_seconds: number;
  errors: number;
  kill_switch_active: boolean;
  last_address: string;
  last_coin: string;
  last_phrase: string;
  last_balance: number;
}

export const EMPTY_STATS: ScanStats = {
  running: false, workers: 0, total_checked: 0, hits: 0, empty: 0,
  received_but_empty: 0, cache_size: 0, seen_size: 0, checks_per_hour: 0,
  uptime_seconds: 0, errors: 0, kill_switch_active: false,
  last_address: '—', last_coin: '—', last_phrase: '—', last_balance: 0,
};

export interface HitRecord {
  id: string;
  time: string;
  coin: string;
  address: string;
  phrase: string;
  source: string;
  balance: number;
  received: number;
}

export interface LogEvent { time: string; kind: string; text: string }
export interface EndpointHealth {
  name: string; coin: string; ok: number; fail: number;
  quarantined_for_seconds: number; last_error: string;
}

export interface UpdateInfo {
  available: boolean;
  version: string;
  current_version: string;
  notes: string;
  url: string;
  asset_name: string;
  size: number;
  checksum: string;
  published_at: string;
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// ------------------------------- browser demo -------------------------------

const mock: {
  config: AppConfig;
  stats: ScanStats;
  hits: HitRecord[];
  events: LogEvent[];
  history: number[];
  timer: number | null;
} = {
  config: { ...DEFAULT_CONFIG },
  stats: { ...EMPTY_STATS },
  hits: [],
  events: [{
    time: new Date().toLocaleTimeString('ru-RU'),
    kind: 'info',
    text: 'Демо-режим браузера. В Tauri приложении данные приходят из Rust-бэкенда.',
  }],
  history: [],
  timer: null,
};

const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function fakeAddress(coin: string): string {
  if (coin === 'ETH' || coin === 'BSC' || coin === 'MATIC' || coin === 'ARB' || coin === 'OPT' || coin === 'BASE' || coin === 'AVAX') {
    return '0x' + Array.from({ length: 40 }, () => Math.floor(Math.random() * 16).toString(16)).join('');
  }
  if (coin === 'SOL') return Array.from({ length: 44 }, () => alphabet[Math.floor(Math.random() * alphabet.length)]).join('');
  if (coin === 'TON') return 'EQ' + Array.from({ length: 46 }, () => alphabet[Math.floor(Math.random() * alphabet.length)]).join('');
  return '1' + Array.from({ length: 33 }, () => alphabet[Math.floor(Math.random() * alphabet.length)]).join('');
}

function tickMock() {
  const s = mock.stats;
  if (!s.running) return;
  const inc = mock.config.workers * Math.max(1, Math.round(mock.config.batch_size / 3));
  s.total_checked += inc;
  s.empty += inc;
  s.uptime_seconds += 1;
  s.checks_per_hour = s.total_checked * 3600 / Math.max(1, s.uptime_seconds);
  s.cache_size = Math.min(s.seen_size + inc, 200000);
  s.seen_size += inc;
  const coin = COINS[Math.floor(Math.random() * COINS.length)];
  s.last_coin = coin;
  s.last_address = fakeAddress(coin);
  s.last_phrase = 'abandon abandon abandon … about';
  s.last_balance = 0;
  mock.history.push(Math.round(s.checks_per_hour));
  if (mock.history.length > 60) mock.history.shift();
  if (Math.random() < 0.03) {
    const hit: HitRecord = {
      id: String(Date.now()), time: new Date().toLocaleTimeString('ru-RU'), coin,
      address: s.last_address, phrase: s.last_phrase, source: mock.config.scan_mode,
      balance: Math.floor(Math.random() * 900000) + 1, received: Math.floor(Math.random() * 1000000),
    };
    mock.hits.unshift(hit);
    s.hits += 1;
    mock.events.unshift({ time: hit.time, kind: 'hit', text: `🎯 ${coin} — ${hit.address}` });
  }
  if (mock.events.length > 100) mock.events.pop();
}

async function mockInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const result = (value: unknown) => value as T;
  switch (cmd) {
    case 'load_config': return result({ ...mock.config });
    case 'save_config': return result([] as string[]);
    case 'export_config': return result(JSON.stringify(mock.config, null, 2));
    case 'import_config': {
      mock.config = { ...mock.config, ...(JSON.parse(String(args.json)) as Partial<AppConfig>) };
      return result({ ...mock.config });
    }
    case 'get_stats': return result({ ...mock.stats });
    case 'get_events': return result([...mock.events]);
    case 'get_history': return result([...mock.history]);
    case 'get_hits': return result([...mock.hits]);
    case 'get_stored_hits': return result([...mock.hits]);
    case 'clear_hits': mock.hits = []; mock.stats.hits = 0; return result('ok');
    case 'start_scanner': {
      mock.config = { ...mock.config, ...(args.config as AppConfig) };
      mock.stats = { ...mock.stats, running: true, workers: mock.config.workers, kill_switch_active: false };
      if (mock.timer === null) mock.timer = window.setInterval(tickMock, 1000);
      return result('Сканер запущен (демо)');
    }
    case 'stop_scanner': {
      mock.stats.running = false;
      if (mock.timer !== null) { window.clearInterval(mock.timer); mock.timer = null; }
      return result('Сканер остановлен');
    }
    case 'test_api': return result('API доступен (демо-режим)');
    case 'endpoint_health': return result([{ name: 'blockstream', coin: 'BTC', ok: 12, fail: 0, quarantined_for_seconds: 0, last_error: '' }]);
    case 'preview_candidates': {
      const count = Number(args.count ?? 3);
      return result(Array.from({ length: count }, (_, i) => {
        const coin = COINS[i % COINS.length];
        return [coin, fakeAddress(coin), 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about'];
      }));
    }
    case 'check_for_updates': return result({
      available: false,
      version: '0.6.1',
      current_version: '0.6.1',
      notes: '',
      url: '',
      asset_name: '',
      size: 0,
      checksum: '',
      published_at: '',
    });
    case 'install_update': return result('Установка обновления доступна только в десктопной версии.');
    case 'set_autostart': mock.config.autostart = Boolean(args.enabled); return result('ok');
    case 'set_stealth_title': return result('ok');
    case 'set_window_visible': return result('ok');
    default: return result(undefined);
  }
}

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isTauri()) return mockInvoke<T>(cmd, args);
  const core = await import('@tauri-apps/api/core');
  return core.invoke<T>(cmd, args);
}

// ------------------------------ typed commands ------------------------------

export const api = {
  loadConfig: () => invoke<AppConfig>('load_config'),
  saveConfig: (config: AppConfig) => invoke<string[]>('save_config', { config }),
  exportConfig: () => invoke<string>('export_config'),
  importConfig: (json: string) => invoke<AppConfig>('import_config', { json }),
  getStats: () => invoke<ScanStats>('get_stats'),
  getEvents: () => invoke<LogEvent[]>('get_events'),
  getHistory: () => invoke<number[]>('get_history'),
  getHits: () => invoke<HitRecord[]>('get_hits'),
  getStoredHits: () => invoke<HitRecord[]>('get_stored_hits'),
  clearHits: () => invoke<string>('clear_hits'),
  start: (config: AppConfig) => invoke<string>('start_scanner', { config }),
  stop: () => invoke<string>('stop_scanner'),
  testApi: (config: AppConfig) => invoke<string>('test_api', { config }),
  endpointHealth: () => invoke<EndpointHealth[]>('endpoint_health'),
  checkUpdates: () => invoke<UpdateInfo>('check_for_updates'),
  installUpdate: (info: UpdateInfo) => invoke<string>('install_update', { info }),
  preview: (config: AppConfig, count = 3) => invoke<[string, string, string][]>('preview_candidates', { config, count }),
  setWindowVisible: (visible: boolean) => invoke<string>('set_window_visible', { visible }),
  setAutostart: (enabled: boolean) => invoke<string>('set_autostart', { enabled }),
  setStealthTitle: (enabled: boolean) => invoke<string>('set_stealth_title', { enabled }),
};
