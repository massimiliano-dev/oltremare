// Visualizzatore degli screenshot: tutte le immagini di un'esecuzione, con le frecce.
class LightboxState {
  images = $state<{ src: string; caption: string }[]>([]);
  index = $state(-1);
  open = $derived(this.index >= 0 && this.index < this.images.length);

  show(images: { src: string; caption: string }[], index: number) {
    this.images = images;
    this.index = index;
  }
  close() {
    this.index = -1;
  }
  move(d: number) {
    if (!this.images.length) return;
    this.index = (this.index + d + this.images.length) % this.images.length;
  }
}

export const lightbox = new LightboxState();
