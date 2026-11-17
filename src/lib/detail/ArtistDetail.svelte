<script lang="ts">
  import { assetUrl, getArtist, type ArtistDetail, type ReleaseInfo } from '../api';

  interface Props {
    artistId: number;
    monogram: string;
    /** Bumped whenever the library changes, to refetch. */
    version: number;
    onClose: () => void;
  }

  let { artistId, monogram, version, onClose }: Props = $props();
  let detail = $state.raw<ArtistDetail | null>(null);
  let error = $state<string | null>(null);
  let closeButton = $state<HTMLButtonElement>();

  $effect(() => {
    const id = artistId;
    void version;
    let alive = true;
    getArtist(id)
      .then((d) => {
        if (alive) detail = d;
      })
      .catch((e) => {
        if (alive) error = String(e);
      });
    return () => {
      alive = false;
    };
  });

  $effect(() => {
    closeButton?.focus();
  });

  const status = $derived.by(() => {
    if (!detail) return '';
    switch (detail.mbStatus) {
      case 'matched':
        return detail.mbVerified ? 'MusicBrainz · verified' : 'MusicBrainz · name match';
      case 'pending':
        return 'Looking up on MusicBrainz…';
      case 'error':
        return 'MusicBrainz lookup failed; will retry';
      default:
        return 'Not found on MusicBrainz';
    }
  });

  function duration(ms: number): string {
    const s = Math.round(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }

  function releaseMeta(r: ReleaseInfo): string {
    const parts: string[] = [];
    if (r.year) parts.push(String(r.year));
    if (r.mbType) parts.push(r.mbType);
    parts.push(`${r.tracks.length} ${r.tracks.length === 1 ? 'track' : 'tracks'}`);
    const total = r.tracks.reduce((sum, t) => sum + t.durationMs, 0);
    parts.push(duration(total));
    return parts.join(' · ');
  }

  function multiDisc(r: ReleaseInfo): boolean {
    return r.tracks.some((t) => (t.discNo ?? 1) > 1);
  }
</script>

<div class="scrim" role="presentation" onclick={onClose}></div>
<div class="panel" role="dialog" aria-modal="true" aria-label={detail?.name ?? 'Artist'}>
  <header class="head">
    {#if detail?.headshotPath}
      <img class="photo" src={assetUrl(detail.headshotPath)} alt="" />
    {:else}
      <div class="photo monogram">{monogram}</div>
    {/if}
    <div class="titles">
      <h1>{detail?.name ?? ''}</h1>
      {#if detail}
        <div class="sub">
          {#if detail.disambiguation}<span>{detail.disambiguation}</span> ·{/if}
          <span title={detail.mbid ?? undefined}>{status}</span>
        </div>
      {/if}
    </div>
    <button bind:this={closeButton} class="close" onclick={onClose} aria-label="Close">Esc</button>
  </header>

  <div class="body">
    {#if error}
      <p class="muted">{error}</p>
    {:else if detail}
      {#each detail.releases as release (release.id)}
        <section class="release">
          <div class="cover">
            {#if release.coverThumbPath}
              <img src={assetUrl(release.coverThumbPath)} alt="" loading="lazy" decoding="async" />
            {/if}
          </div>
          <div class="tracks">
            <h2>{release.title}</h2>
            <div class="muted">{releaseMeta(release)}</div>
            <ol>
              {#each release.tracks as t, i (t.id)}
                {#if multiDisc(release) && (i === 0 || release.tracks[i - 1].discNo !== t.discNo)}
                  <li class="disc">Disc {t.discNo ?? 1}</li>
                {/if}
                <li class="track">
                  <span class="no">{t.trackNo ?? ''}</span>
                  <span class="title">
                    {t.title}
                    {#if t.artist !== detail.name}<span class="muted"> — {t.artist}</span>{/if}
                  </span>
                  <span class="dur">{duration(t.durationMs)}</span>
                </li>
              {/each}
            </ol>
          </div>
        </section>
      {/each}
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: rgb(0 0 0 / 0.55);
  }
  .panel {
    position: fixed;
    z-index: 11;
    top: 48px;
    bottom: 48px;
    left: 50%;
    width: min(880px, calc(100vw - 96px));
    transform: translateX(-50%);
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    border-radius: 20px;
    background: #141418;
    box-shadow:
      0 0 0 1px rgb(255 255 255 / 0.08),
      0 24px 80px rgb(0 0 0 / 0.6);
    overflow: hidden;
    user-select: text;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 24px 28px;
    border-bottom: 1px solid rgb(255 255 255 / 0.06);
  }
  .photo {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    object-fit: cover;
    flex: none;
    background: #24242a;
  }
  .monogram {
    display: grid;
    place-items: center;
    font-size: 24px;
    font-weight: 650;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .sub {
    margin-top: 4px;
    color: rgb(255 255 255 / 0.55);
  }
  .close {
    flex: none;
    height: 32px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid rgb(255 255 255 / 0.12);
    background: transparent;
    color: rgb(255 255 255 / 0.7);
    font: inherit;
    cursor: pointer;
  }
  .body {
    overflow-y: auto;
    padding: 8px 28px 28px;
    overscroll-behavior: contain;
  }
  .release {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    gap: 24px;
    padding: 20px 0;
    border-bottom: 1px solid rgb(255 255 255 / 0.05);
    content-visibility: auto;
    contain-intrinsic-size: auto 320px;
  }
  .cover {
    width: 160px;
    height: 160px;
    border-radius: 10px;
    overflow: hidden;
    background: #24242a;
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 650;
  }
  .muted {
    color: rgb(255 255 255 / 0.5);
  }
  ol {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
  }
  .track {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    gap: 8px;
    padding: 6px 0;
    border-radius: 6px;
  }
  .track:hover {
    background: rgb(255 255 255 / 0.04);
  }
  .no,
  .dur {
    color: rgb(255 255 255 / 0.45);
    font-variant-numeric: tabular-nums;
  }
  .no {
    text-align: right;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .disc {
    margin-top: 12px;
    padding: 4px 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgb(255 255 255 / 0.45);
  }
</style>
