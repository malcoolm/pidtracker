// Service worker PID Trackeru. __BUILD_ID__ nahrazuje `cargo xtask dist`.
const BUILD_ID = '__BUILD_ID__';
const APP_CACHE = `app-${BUILD_ID}`;
const DATA_CACHE = 'pid-data';

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith('app-') && key !== APP_CACHE) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});

async function networkFirst(request, cacheName) {
  const cache = await caches.open(cacheName);
  try {
    const response = await fetch(request, { cache: 'no-store' });
    if (response.ok) await cache.put(request, response.clone());
    return response;
  } catch (err) {
    const cached = await cache.match(request, { ignoreSearch: true });
    if (cached) return cached;
    throw err;
  }
}

async function cacheFirst(request, cacheName) {
  const cache = await caches.open(cacheName);
  const cached = await cache.match(request);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) await cache.put(request, response.clone());
  return response;
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  const path = url.pathname;

  if (request.mode === 'navigate') {
    event.respondWith(networkFirst(new Request(self.registration.scope), APP_CACHE));
  } else if (path.endsWith('/version.json')) {
    event.respondWith(networkFirst(request, APP_CACHE));
  } else if (path.endsWith('/data/manifest.json')) {
    event.respondWith(networkFirst(request, DATA_CACHE));
  } else if (path.includes('/data/') && path.endsWith('.pidt')) {
    event.respondWith(cacheFirst(request, DATA_CACHE));
  } else {
    event.respondWith(cacheFirst(request, APP_CACHE));
  }
});
