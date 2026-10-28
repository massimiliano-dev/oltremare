# Oltremare

Assistente personale desktop per macOS e Windows. Ha un volto animato, ascolta e parla, e lancia agenti che lavorano al posto tuo sul computer: file, comandi, app, schermo, browser, posta, calendario, attività, notifiche e sviluppo software. I modelli sono a scelta: **Claude** (chiave API o il tuo abbonamento Pro/Max), **GPT** o un'**AI locale** che gira sul computer.

## Avvio

```bash
npm install
npm run tauri dev        # sviluppo
npm run tauri build      # pacchetti di release (.app/.dmg, .msi/.exe)
```

Per installarla sul Mac usa `./scripts/installa-mac.sh`. Lo script compila la versione firmata con il tuo certificato «Apple Development» e la copia in Applicazioni. Con la stessa firma macOS conserva i permessi (schermo, accessibilità, disco, microfono) da una versione all'altra; con una build non firmata andrebbero riconcessi ogni volta.

Requisiti: Rust ≥ 1.88 e Node ≥ 20. Non serve cmake: i motori di voce e AI locale arrivano già compilati.

Al primo avvio apri **Impostazioni → Provider** (`⌘,`) e scegli come parlare con il modello:

- **Abbonamento Claude (Pro o Max)**: se Claude Code è installato e collegato al tuo account, Oltremare lo usa così com'è. Nessuna chiave API e nessun costo per richiesta; valgono i limiti del piano. Pensato per uso personale su questo computer. Oltremare non legge mai le credenziali di Claude Code: avvia il programma ufficiale.
- **Chiave API di Claude** (console.anthropic.com), a consumo.
- **Chiave di OpenAI** o di un servizio compatibile.
- **AI locale**: in **Impostazioni → AI locale** scarichi un modello con un clic.

Se non c'è nessuna chiave API ma Claude Code è collegato, gli agenti passano da soli all'abbonamento.

## Cosa fa

### Agenti
- Nome, icona, colore, istruzioni, provider, modello e livello di impegno per ciascuno. `⌘⇧A` crea, `⌘E` modifica.
- Agenti già pronti:
  - **Assistente**: sempre presente, con file, comandi, app, posta, calendario, attività e notifiche.
  - **Briefing**: il briefing del mattino.
  - **Sviluppatore**: lavora sul codice, descritto più sotto.
- **Strumenti con approvazione**: le letture partono da sole, le azioni che modificano qualcosa chiedono conferma. La regola (Consenti, Chiedi, Nega) si sceglie per agente e per strumento. Nella richiesta di conferma ci sono anteprima e differenze dei file. Si risponde da tastiera: `↵` consente, `⇧↵` consente per tutta l'esecuzione, `Esc` nega. Per le azioni ad alto rischio serve `⌘↵`.
- **Interruttore di emergenza**: `⌘⇧Esc` (su Windows `Ctrl+Alt+Esc`) ferma tutto. Nega le richieste aperte, interrompe gli agenti, zittisce la voce e rilascia mouse e tastiera.
- **Registro delle attività**: ogni azione con la decisione presa e da chi.
- **Avvio automatico** degli agenti, oltre a chat e voce:
  - a orario, con le espressioni cron classiche e il recupero se il computer era spento;
  - quando arrivano file in una cartella, raggruppati in un solo avvio;
  - quando arriva un'email da un mittente o con un oggetto;
  - quando arriva una notifica da un'app o con un testo;
  - con una scorciatoia globale, passando il testo copiato o aprendo la palette.

### Strumenti sul computer
- **File**: leggere, elencare, cercare, scrivere, spostare. Si cestina, non si cancella mai. Si lavora solo nelle cartelle consentite; chiavi, portachiavi e cartelle di sistema restano sempre escluse.
- **Comandi**: zsh su macOS, PowerShell su Windows. Hanno un tempo massimo, l'uscita limitata e un elenco di comandi vietati.
- **App senza schermo**: gli agenti comandano le app del Mac con AppleScript o JavaScript for Automation, in background e senza muovere il mouse. Funziona con Note, Promemoria, Calendario, Mail, Finder, Safari, Numbers, Excel e molte altre. Lo script si approva prima che parta.
- **App, link e appunti**.
- **Schermo** (computer use), solo come ultima risorsa:
  - aspetta che tu lasci mouse e tastiera da qualche secondo (si può spegnere in Impostazioni → Generale);
  - con Claude usa il toolset nativo, con GPT e i modelli locali delle funzioni equivalenti;
  - si ferma se muovi il mouse;
  - non tocca le app di password e banca.
- **Browser**: Chrome o Edge con un profilo separato dal tuo, di norma invisibile: nessuna finestra sul tuo schermo, le pagine si vedono negli screenshot dei passaggi. In Impostazioni → Generale si può mostrare. Lavora su riferimenti agli elementi della pagina e non compila mai i campi password. Si può limitare a certi siti.
- **Contenuti**:
  - **PDF** da Markdown o HTML, impaginati con il Chrome già usato dagli agenti, senza programmi in più.
  - **Presentazioni** 16:9 in PDF, PowerPoint modificabile o HTML, con [Marp CLI](https://github.com/marp-team/marp-cli) (`npm i -g @marp-team/marp-cli`). Senza Marp si fanno comunque PDF e HTML.
  - **Immagini** gratis e in locale con [Draw Things](https://drawthings.ai). Si installa dal Mac App Store e si attiva in Impostazioni → API Server, porta 7860. Se Draw Things non è acceso si usa OpenAI, a pagamento, se c'è la chiave.
  - **Video** MP4 da immagini, con dissolvenze e audio, tramite FFmpeg (`brew install ffmpeg`).
  - I file vanno in Documenti/Oltremare se non si indica un percorso.
- **Allegati in chat**: PDF, immagini, Excel, Word, PowerPoint, CSV, XML, JSON, testo. Si trascinano o si scelgono con la graffetta.

### Voce
- «**Ehi Oltremare**» oppure un tasto da tenere premuto (`⌥⇧Spazio`).
- Tutto in locale:
  - rilevamento della voce con Silero;
  - trascrizione con Whisper;
  - voce neurale con Piper (voci italiane Paola e Miro).
- L'avatar muove la bocca mentre parla. Le richieste di conferma si possono approvare a voce («sì» o «no»).
- Ogni agente può rispondere a voce sempre, solo quando gli parli o mai. Delle frasi possono attivare un agente specifico.

### Avatar
- Due volti con gli stessi stati:
  - il **Personaggio**, un piccolo robot con espressioni;
  - l'**Orb**, una sfera di luce.
- Nella finestra o come **compagno** sullo schermo, una finestrella sempre in primo piano: clic per parlare, doppio clic per aprire Oltremare.

### AI locale
Oltremare scarica `llama-server` (llama.cpp, build fissata e verificata con sha256) e i modelli GGUF da Hugging Face, con ripresa del download. Gratis, e i dati non lasciano il computer. Modelli consigliati:

| Modello | Per chi |
|---|---|
| Gemma 4 26B (MoE) | consigliato con 32 GB: veloce, ottimo italiano |
| Gemma 4 12B | da 16 GB, vede le immagini |
| Gemma 4 E4B, Qwen 3.5 4B | computer con 8–16 GB |
| Qwen 3.8 27B | il più preciso con gli strumenti, più lento |

Si può anche importare un file GGUF già scaricato.

### Account: posta, calendario, attività, contatti
Microsoft 365 e Outlook.com tramite Microsoft Graph, Gmail e Google tramite le loro API. Gli agenti cercano, leggono, scrivono bozze e inviano le email, sempre con conferma. Gestiscono anche eventi, attività (To Do, Google Tasks) e contatti. Le istruzioni per registrare le app sono più sotto.

### Notifiche e briefing
- **Fonti**:
  - nuove email;
  - chat di **Teams** (account di lavoro o scuola);
  - **Slack**;
  - **Centro Notifiche di macOS**: tutte le app, comprese WhatsApp e Messaggi. Serve l'accesso completo al disco.
- **Regole**: per app e testo, una notifica può avvisarti, farsi leggere a voce o passare a un agente. Ci sono fasce di silenzio. Con «le notifiche restano sul computer» solo gli agenti con AI locale possono leggerle.
- **Briefing del mattino**:
  - all'orario e nei giorni scelti, o appena accendi il computer (entro le 13);
  - raccoglie impegni di oggi e domani, attività, email da leggere, notifiche della sera prima e lavori di Oltremare in sospeso;
  - lo legge a voce in meno di due minuti.

  «Briefing adesso» è nel pannello Notifiche e nel menu dell'icona.

### Sviluppo e strumenti esterni (MCP)
- **Server MCP**: in **Impostazioni → Strumenti esterni** colleghi server MCP via HTTP o come programmi locali. I loro strumenti diventano strumenti degli agenti, con le stesse regole di approvazione. Le istruzioni del server arrivano all'agente che li usa. «**Importa da Claude Code**» copia i server già configurati lì, con lo stesso metodo di autenticazione (`headersHelper`). Le intestazioni fisse e le variabili d'ambiente finiscono nel portachiavi.
- **Claude nell'editor**:
  - «Scrivi a Claude nell'editor» porta in primo piano il progetto in VS Code (o in un'app derivata), scrive il messaggio nel pannello di Claude Code e preme Invio, come faresti tu.
  - «Segui Claude» legge la conversazione salvata da Claude Code e dice cosa sta facendo, se ha finito, se fa una domanda o se è fermo su una richiesta di permesso. Con un'attesa, aspetta la fine senza consumare passi.
  - I permessi nel pannello si danno guardando lo schermo e cliccando, con le stesse regole che useresti tu.
- **Vista Sviluppo**: è lo spazio di lavoro dello Sviluppatore. Si apre selezionando l'agente, con `⌘⇧D` o con l'icona del terminale in alto; è un piccolo IDE dentro Oltremare.
  - A destra c'è la conversazione con lo Sviluppatore, che riceve anche il progetto aperto. Accanto c'è quella di Claude sul progetto.
  - Albero dei file, con i file cambiati evidenziati; visualizzatore di file e differenze git.
  - La conversazione di Claude dal vivo, sia nel pannello dell'editor sia in background, con un campo per scrivergli «Nell'editor» o «Qui».
  - I processi con i loro log e l'anteprima dell'app sull'indirizzo locale.
- **Ripiego automatico**: se non si riesce a scrivere a Claude nell'editor (editor assente, permessi, messaggio non arrivato), Claude Code lavora in background sullo stesso progetto e la stessa conversazione, e lo segui nella vista Sviluppo. In Impostazioni → Strumenti esterni puoi scegliere di lavorare sempre in Oltremare.
- **Processi in background**: backend, frontend e watcher restano accesi mentre l'agente lavora. L'agente ne legge i log e gli indirizzi locali e li ferma alla fine; si chiudono comunque con l'app.
- **Prove nel browser**: l'agente apre l'app sull'indirizzo locale, clicca, compila e controlla che la funzione richiesta funzioni.
- **Sviluppa in background** (facoltativo): affida un lavoro a Claude Code senza editor e restituisce riepilogo, stato git e sessione. Lo Sviluppatore non lo usa.
- L'agente **Sviluppatore** lavora come te:
  1. capisce il progetto e usa il gestionale delle issue secondo le sue istruzioni;
  2. scrive a Claude nell'editor e lo segue, rispondendo ai permessi;
  3. avvia backend e frontend e prova nel browser;
  4. riporta a Claude gli errori finché tutto funziona;
  5. riassume il lavoro, senza push.

### Isola

Il compagno può essere un'**isola**: una pillola nera in alto al centro. Sui Mac con la notch le sta attorno, come la Dynamic Island; sugli altri resta appena sotto la barra dei menu. A riposo è piccola. Si allarga con un'animazione quando Oltremare ascolta (onda della voce e trascrizione), lavora (agente e passo in corso) o parla. Si allarga anche quando chiede un'approvazione, con i pulsanti Consenti e Nega, o quando arriva un avviso. Al passaggio del mouse mostra Parla, Chiedi e Apri; un clic apre la palette. Lo stile si sceglie in Impostazioni → Aspetto.

### Finestre e interfaccia
- **Icona nella barra dei menu** (area di notifica su Windows): segue lo stato dell'avatar e mostra un segno quando serve la tua attenzione. Dal menu: palette, briefing, compagno, ascolto, ferma tutto, avvio automatico, esci. Chiudere la finestra non spegne Oltremare.
- **Palette** da qualsiasi app (`⌥Spazio`): scrivi, scegli l'agente con «/», la risposta arriva lì. Puoi approvare al volo, e `⌘↵` apre la conversazione completa.
- **Avvio all'accensione**, nella barra dei menu e senza finestra. Una sola istanza alla volta.
- **Aspetto**:
  - tema chiaro, scuro o di sistema;
  - otto colori d'accento più tonalità e intensità libere;
  - densità comoda o compatta e dimensione del testo.
- **Disposizione**: Operatore, Conversazione, Monitor o Concentrazione. I bordi dei pannelli si trascinano, anche da tastiera, e le larghezze si ricordano. `⌘B` mostra o nasconde gli agenti, `⌘J` il pannello laterale.
- **Tastiera**: `⌘K` comandi, `⌘N` nuova conversazione, `⌘L` scrivi, `⌘.` ferma, `⌘1…9` cambia agente, `⌘,` impostazioni.

## Registrare le app per gli account (gratis, una volta)

Gli ID si inseriscono in **Impostazioni → Account → Applicazioni OAuth**. In alternativa si mettono in `src-tauri/.cargo/config.toml` (copia di `config.toml.example`, escluso da git) come `OLTREMARE_MS_CLIENT_ID`, `OLTREMARE_GOOGLE_CLIENT_ID` e `OLTREMARE_GOOGLE_CLIENT_SECRET`, e poi si ricompila. I token stanno nel portachiavi; l'accesso usa PKCE con redirect su `http://localhost`.

### Microsoft
1. Apri [portal.azure.com](https://portal.azure.com) e vai su **Microsoft Entra ID → Registrazioni app → Nuova registrazione**. Basta un account Microsoft personale: il tenant gratuito va bene.
   - Nome: `Oltremare`.
   - Tipi di account: *qualsiasi directory organizzativa e account Microsoft personali*.
   - URI di reindirizzamento: **Client pubblico/nativo** con valore `http://localhost`.
2. Copia l'**ID applicazione (client)**.
3. Oltremare chiede questi permessi delegati al primo accesso: Mail.ReadWrite, Mail.Send, Calendars.ReadWrite, Contacts.Read, Tasks.ReadWrite, User.Read e offline_access. Per Teams chiede anche Chat.Read; vale solo con account di lavoro o scuola, e alcune organizzazioni richiedono il consenso di un amministratore.

### Google
1. Apri [console.cloud.google.com](https://console.cloud.google.com) e crea un nuovo progetto. Nella **schermata consenso OAuth** scegli Esterno e aggiungi il tuo indirizzo tra gli utenti di test.
2. In **API e servizi → Libreria**, abilita Gmail API, Google Calendar API, Google Tasks API e People API.
3. In **Credenziali → Crea ID client OAuth** scegli il tipo **App desktop**. Copia ID e secret; per le app desktop Google considera pubblico il secret.

Finché l'app Google è in modalità test, l'accesso scade ogni 7 giorni e va ricollegato.

### Slack (facoltativo)
Crea un'app su [api.slack.com/apps](https://api.slack.com/apps). Aggiungi gli ambiti utente `channels:history`, `groups:history`, `im:history`, `mpim:history` e `users:read`, e installala nel workspace. Poi incolla lo «User OAuth Token» (`xoxp-…`) in **Impostazioni → Notifiche**.

## Permessi di macOS

Si concedono in **Impostazioni di Sistema → Privacy e sicurezza**. Oltremare li chiede solo quando servono.

| Permesso | Serve per |
|---|---|
| Microfono | voce |
| Registrazione schermo | vedere lo schermo |
| Accessibilità | usare mouse e tastiera |
| Accesso completo al disco | leggere il Centro Notifiche (solo se attivi quella fonte) |

In sviluppo ogni ricompilazione può far perdere i permessi: vanno riconcessi.

## Sicurezza e privacy

- Chiavi API, token degli account, token di Slack e segreti dei server MCP stanno nel **portachiavi di sistema**, mai in chiaro su disco.
- Le chiamate ai modelli e ai servizi partono dal backend Rust: niente segreti nella finestra.
- Contenuti di email, pagine web, file e notifiche sono presentati agli agenti come dati, non come istruzioni. Le azioni che inviano, cancellano o pubblicano restano da approvare.
- La voce è elaborata solo in locale e l'audio non viene salvato.
- Con Claude la cronologia cresce solo in coda, e il contenuto delle risposte è rimandato identico, come richiede l'API.

## Dove stanno i dati

- **Impostazioni e agenti**: nella cartella di configurazione dell'app (`settings.json`, `agenti/<id>.json`).
- **Database** `oltremare.db` (SQLite), nella cartella dati: conversazioni, esecuzioni, passi, registro delle attività e notifiche, che si conservano 30 giorni.
- **Modelli locali, voce e motore llama.cpp**: nella cartella dati dell'app (`modelli/`, `llama/`, `voce/`).
- **Profilo del browser degli agenti**: `browser/` nella cartella dati. Si azzera dalle impostazioni.

## Anteprima della grafica

`npm run dev` aperto nel browser, fuori da Tauri, mostra l'app con dati di prova (`src/dev/preview.ts`). Scenari disponibili: `?scenario=empty`, `?scenario=error` e `?scenario=dark`. La palette si apre con `?view=palette`, il compagno con `?view=avatar`.

## Struttura

```
src-tauri/src/
  lib.rs              comandi Tauri, avvio, scorciatoie globali
  finestre.rs         icona nella barra dei menu, palette, avvio automatico
  store.rs            impostazioni e agenti (JSON), segreti (portachiavi)
  db.rs               SQLite (migrazioni con user_version)
  agent/              ciclo dell'agente, strumenti, policy, approvazioni, timeline, costi
  providers/          anthropic, claude_code (abbonamento), openai_compat (GPT e llama), sse
  tools/              fs, shell, apps, appunti, computer, browser, posta, sviluppo
  mcp.rs              server MCP locale (strumenti di Oltremare per Claude Code)
  mcp_client.rs       client MCP (strumenti dei server esterni)
  sidecar/, modelli/  llama-server e download dei modelli
  voce/               cattura, VAD, trascrizione, parola di attivazione, sintesi
  notifiche/          macOS, Teams, Slack, posta; regole e avvisi
  briefing/           raccolta dei dati e avvio del briefing
  triggers/           orari (cron), cartelle osservate, email e notifiche
  avatar/             stato unico dell'avatar
src/
  App.svelte, PaletteApp.svelte, CompanionApp.svelte
  lib/state/          stato (Svelte 5 runes)
  lib/panels/         Agenti, Chat, Avatar, Passaggi/Attività/Notifiche
  lib/agents/         editor degli agenti, strumenti, avvii automatici
  lib/settings/       impostazioni
  lib/avatar/, lib/run/, lib/chat/, lib/voice/
```

## Test e controlli

- **Backend**: `cd src-tauri && cargo test`.
- **Prove reali**: con `-- --ignored` partono anche quelle vere, con Claude Code e con i server MCP configurati; consumano pochi token del piano.
- **Frontend**: `npm run check`.

## Limiti noti

- Le notifiche di sistema su **Windows** non sono ancora lette: serve un'identità di pacchetto MSIX. Teams, Slack e posta funzionano.
- La parola di attivazione usa la trascrizione: in ambienti rumorosi conviene il tasto.
- La build per Windows va fatta e provata su Windows (o in CI); l'installer non firmato fa scattare SmartScreen.
- I modelli locali piccoli usano gli strumenti meno bene dei modelli grandi.

## Licenza

[MIT](LICENSE). Modelli, voci e motori scaricati al primo uso (llama.cpp, modelli GGUF, Whisper, Piper, sherpa-onnx) restano sotto le rispettive licenze, indicate nelle loro pagine.
