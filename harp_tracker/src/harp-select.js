/// animated and themable dropdown menu
class HarpSelect extends HTMLElement {
  static get observedAttributes() {
    return ['disabled'];
  }

  constructor() {
    super();
    this._value = null;
    this._isOpen = false;
    this._ready = false;
    this._activeIndex = 0;
    this._options = [];
    this._optionEls = [];
    this._portal = null;
    this._uid = Math.random().toString(36).slice(2);

    this._onDocPointerDown = this._onDocPointerDown.bind(this);
    this._onScrollOrResize = this._onScrollOrResize.bind(this);
    this._onKeydown = this._onKeydown.bind(this);
  }

  /* ------------------------------- data ------------------------------- */

  _parseOptions() {
    const nodes = Array.from(this.querySelectorAll('option, harp-option'));
    this._options = nodes.map((el) => ({
      value: el.getAttribute('value') ?? (el.textContent || '').trim(),
      label: (el.textContent || '').trim(),
      selected:
        el.hasAttribute('selected') ||
        el.getAttribute('aria-selected') === 'true',
    }));
  }

  get options() {
    return this._options.map((o) => ({ ...o }));
  }

  get value() {
    return this._value;
  }

  set value(v) {
    if (!this._options.some((o) => o.value === v)) return;
    this._value = v;
    this._syncUI();
  }

  get selectedIndex() {
    return this._options.findIndex((o) => o.value === this._value);
  }

  get disabled() {
    return this.hasAttribute('disabled');
  }

  set disabled(v) {
    this.toggleAttribute('disabled', !!v);
  }

  attributeChangedCallback(name) {
    if (name === 'disabled') this._applyDisabled();
  }

  /* ---------------------------- lifecycle ---------------------------- */

  connectedCallback() {
    if (this._ready) return;
    this._ready = true;

    this._parseOptions();

    if (this._value == null) {
      const def =
        this._options.find((o) => o.selected) || this._options[0] || null;
      this._value = def ? def.value : null;
    }

    this._buildToggle();
    this._syncUI();
    this._bind();
    this._applyDisabled();
  }

  disconnectedCallback() {
    this._close();
    document.removeEventListener('pointerdown', this._onDocPointerDown);
    window.removeEventListener('scroll', this._onScrollOrResize, true);
    window.removeEventListener('resize', this._onScrollOrResize);
  }

  /* ------------------------------- UI ------------------------------- */

  _buildToggle() {
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.className = 'harp-dd-toggle';
    toggle.setAttribute('aria-haspopup', 'listbox');
    toggle.setAttribute('aria-expanded', 'false');
    toggle.setAttribute('aria-owns', `harp-dd-${this._uid}`);
    toggle.setAttribute(
      'aria-label',
      this.getAttribute('name') || this.id || 'Select value',
    );
    this._toggle = toggle;

    const value = document.createElement('span');
    value.className = 'harp-dd-value';
    this._valueEl = value;

    const chevron = document.createElement('span');
    chevron.className = 'harp-dd-chevron';
    chevron.setAttribute('aria-hidden', 'true');
    chevron.innerHTML =
      '<svg viewBox="0 0 24 24" width="1.05em" height="1.05em" fill="none" ' +
      'stroke="currentColor" stroke-width="2.4" stroke-linecap="round" ' +
      'stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>';

    toggle.append(value, chevron);
    this.append(toggle);
  }

  _buildPortal() {
    const ul = document.createElement('ul');
    ul.className = 'harp-dd-portal';
    ul.setAttribute('role', 'listbox');
    ul.id = `harp-dd-${this._uid}`;

    this._optionEls = this._options.map((opt, i) => {
      const li = document.createElement('li');
      li.className = 'harp-dd-option';
      li.setAttribute('role', 'option');
      li.dataset.value = opt.value;
      li.style.animationDelay = `${Math.min(i * 0.025, 0.16)}s`;

      const text = document.createElement('span');
      text.className = 'harp-dd-text';
      text.textContent = opt.label;

      const check = document.createElement('span');
      check.className = 'harp-dd-check';
      check.setAttribute('aria-hidden', 'true');
      check.textContent = '\u2713';

      li.append(text, check);
      return li;
    });

    ul.append(...this._optionEls);

    ul.addEventListener('click', (e) => {
      const li = e.target.closest('.harp-dd-option');
      if (li) this._choose(this._optionEls.indexOf(li));
    });
    ul.addEventListener('mousemove', (e) => {
      const li = e.target.closest('.harp-dd-option');
      if (li) this._setActive(this._optionEls.indexOf(li));
    });

    return ul;
  }

  _syncUI() {
    if (!this._valueEl) return;
    const opt = this._options.find((o) => o.value === this._value);
    this._valueEl.textContent = opt ? opt.label : '';
    this._optionEls.forEach((el, i) => {
      const selected = this._options[i].value === this._value;
      el.classList.toggle('selected', selected);
      el.setAttribute('aria-selected', String(selected));
    });
  }

  _applyDisabled() {
    if (this._toggle) this._toggle.setAttribute('aria-disabled', String(this.disabled));
  }

  /* ---------------------------- open/close ---------------------------- */

  _open() {
    if (this.disabled || this._isOpen || !this._options.length) return;

    const portal = this._buildPortal();
    document.body.appendChild(portal);
    this._portal = portal;
    this._positionPortal();

    this._activeIndex = Math.max(0, this.selectedIndex);
    this._markActive();

    this.classList.add('open');
    this._toggle.setAttribute('aria-expanded', 'true');

    // start collapsed, then let it transition open on the next frame
    requestAnimationFrame(() => {
      if (this._portal === portal) portal.classList.add('open');
    });

    document.addEventListener('pointerdown', this._onDocPointerDown);
    window.addEventListener('scroll', this._onScrollOrResize, true);
    window.addEventListener('resize', this._onScrollOrResize);

    this._isOpen = true;
  }

  _close() {
    if (!this._isOpen) return;
    this._isOpen = false;

    this.classList.remove('open');
    this._toggle.setAttribute('aria-expanded', 'false');

    const portal = this._portal;
    this._portal = null;
    if (portal) {
      portal.classList.remove('open');
      setTimeout(() => portal.remove(), 200);
    }

    document.removeEventListener('pointerdown', this._onDocPointerDown);
    window.removeEventListener('scroll', this._onScrollOrResize, true);
    window.removeEventListener('resize', this._onScrollOrResize);
  }

  _toggleOpen() {
    if (this._isOpen) this._close();
    else this._open();
  }

  _positionPortal() {
    const portal = this._portal;
    if (!portal) return;

    const rect = this._toggle.getBoundingClientRect();
    const estimatedHeight = Math.min(
      240,
      this._optionEls.length * 34 + 24,
    );
    const gap = 6;
    const spaceBelow = window.innerHeight - rect.bottom;
    const openUp = spaceBelow < estimatedHeight && rect.top > estimatedHeight + gap;

    portal.classList.toggle('up', openUp);
    portal.classList.toggle('down', !openUp);
    portal.style.transformOrigin = openUp ? 'bottom center' : 'top center';

    const width = rect.width;
    const maxLeft = Math.max(8, window.innerWidth - width - 8);
    const left = Math.min(Math.max(8, rect.left), maxLeft);
    portal.style.left = `${left}px`;
    portal.style.width = `${width}px`;

    if (openUp) {
      portal.style.top = `${rect.top - gap}px`;
      portal.style.maxHeight = `${Math.max(120, rect.top - gap)}px`;
    } else {
      portal.style.top = `${rect.bottom + gap}px`;
      portal.style.maxHeight = `${Math.max(120, spaceBelow - gap)}px`;
    }
  }

  /* ---------------------------- interaction ---------------------------- */

  _choose(i) {
    if (i < 0 || i >= this._options.length) return;
    const val = this._options[i].value;
    const changed = val !== this._value;

    this._close();
    this._value = val;
    this._syncUI();
    this._toggle.focus();

    if (changed) this.dispatchEvent(new Event('change', { bubbles: true }));
  }

  _setActive(i) {
    if (i < 0 || i >= this._optionEls.length) return;
    this._activeIndex = i;
    this._markActive();
    const el = this._optionEls[i];
    if (el) el.scrollIntoView({ block: 'nearest' });
  }

  _markActive() {
    this._optionEls.forEach((el, i) =>
      el.classList.toggle('active', i === this._activeIndex),
    );
  }

  _moveActive(delta) {
    const n = this._options.length;
    if (!n) return;
    this._setActive((this._activeIndex + delta + n) % n);
  }

  _typeahead(char) {
    const start = this._activeIndex;
    for (let step = 1; step <= this._options.length; step++) {
      const i = (start + step) % this._options.length;
      if (this._options[i].label.toLowerCase().startsWith(char.toLowerCase())) {
        this._setActive(i);
        return;
      }
    }
  }

  /* ------------------------------ events ------------------------------ */

  _bind() {
    this._toggle.addEventListener('click', (e) => {
      e.stopPropagation();
      this._toggleOpen();
    });

    this.addEventListener('keydown', this._onKeydown);
  }

  _onKeydown(e) {
    if (this.disabled) return;

    if (!this._isOpen) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
        e.preventDefault();
        this._open();
      }
      return;
    }

    switch (e.key) {
      case 'Escape':
        this._close();
        break;
      case 'ArrowDown':
        e.preventDefault();
        this._moveActive(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        this._moveActive(-1);
        break;
      case 'Home':
        e.preventDefault();
        this._setActive(0);
        break;
      case 'End':
        e.preventDefault();
        this._setActive(this._options.length - 1);
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        this._choose(this._activeIndex);
        break;
      case 'Tab':
        this._close();
        break;
      default:
        if (e.key.length === 1 && /[a-z0-9]/i.test(e.key)) {
          this._typeahead(e.key);
        }
    }
  }

  _onDocPointerDown(e) {
    if (!this._isOpen) return;
    const inside =
      this.contains(e.target) ||
      (this._portal && this._portal.contains(e.target));
    if (!inside) this._close();
  }

  _onScrollOrResize() {
    if (this._isOpen && this._portal) this._positionPortal();
  }
}

if (!customElements.get('harp-select')) {
  customElements.define('harp-select', HarpSelect);
}
