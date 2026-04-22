import { useMemo, useState } from "react";
          {installedCount === 0 && (
            <div className="empty-state">
              <p>Aucun émulateur installé</p>
              <small>Ajoute tes premiers émulateurs avec le bouton ci-dessus.</small>
            </div>
          )}
        </nav>
      </aside>

      <main className="content">
        <section className="panel hero-panel">
          <p className="eyebrow">Configuration portable</p>
          <h2>Dossiers racine</h2>
          <div className="path-grid">
            <PathCard label="Root" value={paths.root} />
            <PathCard label="Emu" value={paths.emu} />
            <PathCard label="Roms" value={paths.roms} />
            <PathCard label="Saves" value={paths.saves} />
            <PathCard label="Firmware" value={paths.firmware} />
          </div>
        </section>

        <section className="panel">
          <p className="eyebrow">Étape suivante</p>
          <h2>Premier flux MVP</h2>
          <ol>
            <li>Connexion à RomM</li>
            <li>Choix du dossier global</li>
            <li>Installation des émulateurs</li>
            <li>Téléchargement des ROMs / saves / firmware</li>
            <li>Lancement du jeu</li>
          </ol>
        </section>
      </main>

      {showPicker && (
        <div className="modal-backdrop" onClick={() => setShowPicker(false)}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <p className="eyebrow">Installation</p>
                <h3>Choisir les émulateurs</h3>
              </div>
              <button className="ghost-button" onClick={() => setShowPicker(false)}>
                Fermer
              </button>
            </div>

            <div className="picker-list">
              {emulators.map((emu) => (
                <div key={emu.id} className="picker-item">
                  <div>
                    <strong>{emu.name}</strong>
                    <p>{emu.platformLabel}</p>
                  </div>
                  <button className="primary-button" onClick={() => toggleInstall(emu.id)}>
                    {emu.status === "installed" ? "Retirer" : "Installer"}
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface PathCardProps {
  label: string;
  value: string;
}

function PathCard({ label, value }: PathCardProps) {
  return (
    <div className="path-card">
      <small>{label}</small>
      <code>{value}</code>
    </div>
  );
}