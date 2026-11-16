<script lang="ts"></script>

<footer class="player">
  <div class="art"></div>
  <div class="meta">
    <div class="title">Nothing playing</div>
    <div class="sub">Playback arrives in a later milestone</div>
  </div>
</footer>

<style>
  .player {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 24px;
    background: rgb(10 10 12 / 0.55);
    border-top: 1px solid rgb(255 255 255 / 0.06);
  }
  .art {
    width: 44px;
    height: 44px;
    border-radius: 8px;
    background: rgb(255 255 255 / 0.08);
  }
  .title {
    font-weight: 600;
  }
  .sub {
    font-size: 12px;
    color: rgb(255 255 255 / 0.5);
  }
</style>
