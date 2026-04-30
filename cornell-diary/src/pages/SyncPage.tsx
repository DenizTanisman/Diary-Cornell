import { useSyncStore } from '../stores/syncStore';
import { ExportDialog } from '../components/sync/ExportDialog';
import { ImportDialog } from '../components/sync/ImportDialog';
import { QRGenerator } from '../components/sync/QRGenerator';
import { QRScanner } from '../components/sync/QRScanner';
import { CloudSyncPanel } from '../components/sync/CloudSyncPanel';
import { useT } from '../locales';

export function SyncPage() {
  const t = useT();
  const dialog = useSyncStore((s) => s.dialog);
  const openDialog = useSyncStore((s) => s.openDialog);
  const closeDialog = useSyncStore((s) => s.closeDialog);
  const lastResult = useSyncStore((s) => s.lastResult);
  const setLastResult = useSyncStore((s) => s.setLastResult);

  return (
    <div className="page-container">
      <h1>{t('sync.title')}</h1>

      <CloudSyncPanel />

      <div className="sync-card">
        <section className="sync-card__item">
          <h2 className="sync-card__title">{t('sync.exportTitle')}</h2>
          <p className="sync-card__description">{t('sync.exportDescription')}</p>
          <button className="sync-card__button" onClick={() => openDialog('export')}>
            {t('sync.exportAction')}
          </button>
        </section>

        <section className="sync-card__item">
          <h2 className="sync-card__title">{t('sync.importTitle')}</h2>
          <p className="sync-card__description">{t('sync.importDescription')}</p>
          <button className="sync-card__button" onClick={() => openDialog('import')}>
            {t('sync.importAction')}
          </button>
        </section>

        <section className="sync-card__item">
          <h2 className="sync-card__title">{t('sync.qrSendTitle')}</h2>
          <p className="sync-card__description">{t('sync.qrSendDescription')}</p>
          <button className="sync-card__button" onClick={() => openDialog('qr-send')}>
            {t('sync.qrSendAction')}
          </button>
        </section>

        <section className="sync-card__item">
          <h2 className="sync-card__title">{t('sync.qrScanTitle')}</h2>
          <p className="sync-card__description">{t('sync.qrScanDescription')}</p>
          <button className="sync-card__button" onClick={() => openDialog('qr-scan')}>
            {t('sync.qrScanAction')}
          </button>
        </section>
      </div>

      {lastResult ? (
        <p className="empty-state" role="status">
          {t('sync.result', {
            inserted: lastResult.inserted,
            updated: lastResult.updated,
            skipped: lastResult.skipped,
          })}
        </p>
      ) : null}

      {dialog === 'export' ? <ExportDialog onClose={closeDialog} /> : null}
      {dialog === 'import' ? (
        <ImportDialog
          onClose={closeDialog}
          onImported={(r) => setLastResult(r)}
        />
      ) : null}
      {dialog === 'qr-send' ? <QRGenerator onClose={closeDialog} /> : null}
      {dialog === 'qr-scan' ? (
        <QRScanner onClose={closeDialog} onImported={(r) => setLastResult(r)} />
      ) : null}
    </div>
  );
}
