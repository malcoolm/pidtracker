// Service worker PID Trackeru. __BUILD_ID__ a __PRECACHE__ doplňuje `cargo xtask dist`.
const BUILD_ID = '__BUILD_ID__';
const PRECACHE = __PRECACHE__;
const APP_CACHE = `app-${BUILD_ID}`;
const DATA_CACHE = 'pid-data';
// Při slabém signálu (metro) nečekat na síť déle a použít cache.
const NETWORK_TIMEOUT_MS = 3000;

self.addEventListener('install', (event) => {
  // Nová verze se aktivuje až po úplném uložení aplikace – offline start funguje vždy.
  event.waitUntil((async () => {
    const cache = await caches.open(APP_CACHE);
    await cache.addAll(PRECACHE.map((p) => new Request(p, { cache: 'reload' })));
    await self.skipWaiting();
  })());
});

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith('app-') && key !== APP_CACHE) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});

function withTimeout(promise, ms) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timeout')), ms);
    promise.then((v) => { clearTimeout(timer); resolve(v); }, (e) => { clearTimeout(timer); reject(e); });
  });
}

async function networkFirst(request, cacheName) {
  const cache = await caches.open(cacheName);
  const key = request;
  try {
    const response = await withTimeout(fetch(request, { cache: 'no-store' }), NETWORK_TIMEOUT_MS);
    if (response.ok) await cache.put(key, response.clone());
    return response;
  } catch (err) {
    const cached = (await cache.match(key, { ignoreSearch: true })) || (await caches.match(key, { ignoreSearch: true }));
    if (cached) return cached;
    throw err;
  }
}

async function cacheFirst(request, cacheName) {
  const cached = await caches.match(request);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(cacheName);
    await cache.put(request, response.clone());
  }
  return response;
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  const path = url.pathname;

  if (request.mode === 'navigate') {
    // Jednostránková aplikace: každá navigace = stránka v kořeni scope (navigační Request nejde znovu použít s volbami).
    event.respondWith(networkFirst(new Request(self.registration.scope), APP_CACHE));
  } else if (path.endsWith('/version.json')) {
    event.respondWith(fetch(request, { cache: 'no-store' }));
  } else if (path.endsWith('/data/manifest.json')) {
    event.respondWith(networkFirst(request, DATA_CACHE));
  } else if (path.includes('/data/') && path.endsWith('.pidt')) {
    event.respondWith(cacheFirst(request, DATA_CACHE));
  } else {
    event.respondWith(cacheFirst(request, APP_CACHE));
  }
});
