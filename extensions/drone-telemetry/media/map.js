// GPS map for the telemetry cockpit, built on Leaflet (vendored locally).
//
// Shows the vehicle as a heading-rotated arrow marker with a breadcrumb trail.
// Tiles come from OpenStreetMap (requires network; the marker/trail still work
// offline against a blank background). Exposed as `window.VyutaMap`.
(function () {
  "use strict";

  class VehicleMap {
    constructor(elementId) {
      this.map = L.map(elementId, {
        zoomControl: true,
        attributionControl: true,
      }).setView([47.397742, 8.545594], 16);

      L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
        maxZoom: 19,
        attribution: "© OpenStreetMap contributors",
      }).addTo(this.map);

      this.marker = null;
      this.trail = L.polyline([], { color: "#ffcc00", weight: 2 }).addTo(this.map);
      this.points = [];
      this.followed = true;
      // Stop auto-following once the user pans the map themselves.
      this.map.on("dragstart", () => (this.followed = false));
    }

    icon(headingDeg) {
      const rot = typeof headingDeg === "number" ? headingDeg : 0;
      return L.divIcon({
        className: "vehicle-marker",
        html:
          '<div class="vehicle-arrow" style="transform: rotate(' +
          rot +
          'deg)"></div>',
        iconSize: [24, 24],
        iconAnchor: [12, 12],
      });
    }

    update(lat, lon, headingDeg) {
      if (typeof lat !== "number" || typeof lon !== "number") return;
      const ll = [lat, lon];
      if (!this.marker) {
        this.marker = L.marker(ll, { icon: this.icon(headingDeg) }).addTo(this.map);
      } else {
        this.marker.setLatLng(ll);
        this.marker.setIcon(this.icon(headingDeg));
      }
      // Append to the trail, capped to a reasonable length.
      this.points.push(ll);
      if (this.points.length > 2000) this.points.shift();
      this.trail.setLatLngs(this.points);
      if (this.followed) this.map.panTo(ll, { animate: false });
    }

    invalidate() {
      // Call after the container becomes visible/resized.
      this.map.invalidateSize();
    }
  }

  window.VyutaMap = VehicleMap;
})();
