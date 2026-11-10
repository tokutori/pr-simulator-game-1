//! Peer-scoped original-byte transfers and their existing storage completions.

use super::*;

pub(super) struct PendingDownload {
    peer: ApplicationPeerId,
    transfer_id: u64,
}

pub(super) struct PendingPicker {
    peer: ApplicationPeerId,
    actor: ActorScope,
    document: ConfigurationDocumentKind,
    maximum_bytes: u32,
}

pub(super) struct PendingUpload {
    picker: PendingPicker,
    request_id: u32,
    manifest: TransferManifest,
    bytes: Vec<u8>,
}

impl ApplicationServer {
    pub(super) fn client_download_pending(&self, id: ApplicationPeerId) -> bool {
        self.downloads.values().any(|pending| pending.peer == id)
    }

    pub(super) fn begin_picker(
        &mut self,
        id: ApplicationPeerId,
        actor: ActorScope,
        document: ConfigurationDocumentKind,
        request_id: u32,
        maximum_bytes: u32,
    ) -> Result<(), ApplicationServerError> {
        let peer = self
            .peers
            .get(&id)
            .ok_or(ApplicationServerError::Admission(
                "picker connection is retired",
            ))?;
        if self.operator != Some(id)
            || peer.role != ApplicationPeerRole::Operator
            || peer.actor() != actor
            || maximum_bytes as usize != document.maximum_source_bytes()
            || self.pickers.contains_key(&request_id)
            || self
                .uploads
                .values()
                .any(|upload| upload.request_id == request_id)
        {
            return Err(ApplicationServerError::Admission(
                "picker does not match its issued operator operation",
            ));
        }
        let request = self
            .authority
            .storage
            .pick_configuration(document, request_id, &self.authority.application)
            .ok_or(ApplicationServerError::Admission(
                "configuration selection is unavailable",
            ))?;
        self.authority
            .shutdown
            .requests(std::slice::from_ref(&request));
        self.pickers.insert(
            request_id,
            PendingPicker {
                peer: id,
                actor,
                document,
                maximum_bytes,
            },
        );
        Ok(())
    }

    pub(super) fn configuration_result(
        &mut self,
        id: ApplicationPeerId,
        document: ConfigurationDocumentKind,
        request_id: u32,
        completion: ConfigurationResult,
    ) -> Result<(), ApplicationServerError> {
        let Some(picker) = self.pickers.get(&request_id) else {
            return Ok(());
        };
        let peer = self
            .peers
            .get(&id)
            .ok_or(ApplicationServerError::Admission(
                "configuration connection is retired",
            ))?;
        if picker.peer != id
            || picker.document != document
            || picker.actor != peer.actor()
            || peer.role != ApplicationPeerRole::Operator
            || self.operator != Some(id)
        {
            return Err(ApplicationServerError::Admission(
                "configuration completion belongs to another operation",
            ));
        }
        let picker = self.pickers.remove(&request_id).expect("admitted picker");
        let completion = match completion {
            ConfigurationResult::Found { transfer } => {
                let expected_kind = if document == ConfigurationDocumentKind::ControllerExecution {
                    BinaryKind::GuestProgram
                } else {
                    BinaryKind::Configuration
                };
                if transfer.kind != expected_kind
                    || transfer.local_id != request_id
                    || transfer.revision != 0
                    || transfer.byte_length > picker.maximum_bytes
                    || transfer.transfer_id == 0
                    || transfer.transfer_id
                        <= self.peers.get(&id).expect("admitted peer").last_upload_id
                {
                    self.finish_configuration(
                        document,
                        request_id,
                        ApplicationConfigurationLoad::Failed {
                            cause:
                                "Configuration transfer does not match its admitted source contract"
                                    .to_owned(),
                        },
                    )?;
                    return Err(ApplicationServerError::Admission(
                        "configuration transfer manifest is invalid",
                    ));
                }
                let mut bytes = Vec::new();
                if bytes
                    .try_reserve_exact(transfer.byte_length as usize)
                    .is_err()
                {
                    self.finish_configuration(
                        document,
                        request_id,
                        ApplicationConfigurationLoad::Failed {
                            cause: "Configuration source allocation failed".to_owned(),
                        },
                    )?;
                    return Err(ApplicationServerError::Capacity);
                }
                self.peers
                    .get_mut(&id)
                    .expect("admitted peer")
                    .last_upload_id = transfer.transfer_id;
                if transfer.byte_length == 0 {
                    return self.finish_configuration(
                        document,
                        request_id,
                        ApplicationConfigurationLoad::Found { bytes },
                    );
                }
                self.uploads.insert(
                    (id, transfer.transfer_id),
                    PendingUpload {
                        picker,
                        request_id,
                        manifest: transfer,
                        bytes,
                    },
                );
                return Ok(());
            }
            ConfigurationResult::Missing => ApplicationConfigurationLoad::Missing,
            ConfigurationResult::Cancelled => ApplicationConfigurationLoad::Cancelled,
            ConfigurationResult::Failed { cause } => ApplicationConfigurationLoad::Failed { cause },
        };
        self.finish_configuration(document, request_id, completion)
    }

    pub(super) fn receive_binary(
        &mut self,
        id: ApplicationPeerId,
        packet: &[u8],
    ) -> Result<(), ApplicationServerError> {
        let (header, data) = BinaryHeader::decode(packet)?;
        let key = (id, header.transfer_id);
        let Some(upload) = self.uploads.get(&key) else {
            if self
                .peers
                .get(&id)
                .is_some_and(|peer| header.transfer_id <= peer.last_upload_id)
            {
                return Ok(());
            }
            return Err(ApplicationServerError::Admission(
                "binary upload has no admitted configuration operation",
            ));
        };
        let peer = self
            .peers
            .get(&id)
            .ok_or(ApplicationServerError::Admission(
                "upload connection is retired",
            ))?;
        let valid = self.operator == Some(id)
            && peer.role == ApplicationPeerRole::Operator
            && upload.picker.actor == peer.actor()
            && header.kind == upload.manifest.kind
            && header.local_id == upload.manifest.local_id
            && header.revision == upload.manifest.revision
            && header.total_len == upload.manifest.byte_length
            && header.offset == upload.bytes.len() as u64;
        if !valid {
            let upload = self.uploads.remove(&key).expect("pending upload");
            self.finish_configuration(upload.picker.document, upload.request_id, ApplicationConfigurationLoad::Failed {
                cause: "Configuration chunk scope, size or order differs from its admitted operation".to_owned() })?;
            return Err(ApplicationServerError::Admission(
                "configuration chunk is invalid",
            ));
        }
        let upload = self.uploads.get_mut(&key).expect("admitted upload");
        upload.bytes.extend_from_slice(data);
        if upload.bytes.len() != upload.manifest.byte_length as usize {
            return Ok(());
        }
        let upload = self.uploads.remove(&key).expect("completed upload");
        let completion = if upload.picker.document == ConfigurationDocumentKind::ControllerExecution
            && !upload.bytes.starts_with(b"\0asm")
        {
            ApplicationConfigurationLoad::Failed {
                cause: "Remote controller execution accepts a Wasm guest program".to_owned(),
            }
        } else {
            ApplicationConfigurationLoad::Found {
                bytes: upload.bytes,
            }
        };
        // The existing selected-configuration validator performs the sole complete decode.
        self.finish_configuration(upload.picker.document, upload.request_id, completion)
    }

    fn finish_configuration(
        &mut self,
        document: ConfigurationDocumentKind,
        request_id: u32,
        completion: ApplicationConfigurationLoad,
    ) -> Result<(), ApplicationServerError> {
        self.complete_io(
            self.authority.observed_ms,
            ApplicationInput::ConfigurationLoaded {
                document,
                request_id,
                completion,
            },
        )
    }

    pub(super) fn deliver_client_request(
        &mut self,
        id: ApplicationPeerId,
        request: ApplicationRequest,
    ) -> Result<(), ApplicationServerError> {
        if !matches!(request, ApplicationRequest::DownloadFile { .. }) {
            return Err(ApplicationServerError::Admission(
                "client transfer request is not an export",
            ));
        }
        self.authority
            .shutdown
            .requests(std::slice::from_ref(&request));
        let ApplicationRequest::DownloadFile {
            request_id,
            file_name,
            mime_type,
            bytes,
        } = request
        else {
            unreachable!("download request checked")
        };
        if self.client_download_pending(id)
            || self
                .peers
                .get(&id)
                .is_none_or(|peer| !matches!(peer.state, PeerState::Ready(_)))
        {
            self.finish_download_storage(
                id,
                request_id,
                Some("Client export is unavailable while a prior submission is pending".to_owned()),
            );
            return Err(ApplicationServerError::Capacity);
        }
        let length = match u32::try_from(bytes.len()) {
            Ok(length) => length,
            Err(_) => {
                self.finish_download_storage(
                    id,
                    request_id,
                    Some("Export bytes exceed the protocol length representation".to_owned()),
                );
                return Err(ApplicationServerError::Capacity);
            }
        };
        let peer = self.peers.get_mut(&id).expect("admitted export peer");
        let transfer_id = match peer.allocate_transfer() {
            Ok(id) => id,
            Err(error) => {
                self.finish_download_storage(id, request_id, Some(error.to_string()));
                return Err(error);
            }
        };
        let scope = peer.scope;
        let transfer = TransferManifest {
            transfer_id,
            kind: BinaryKind::Download,
            local_id: request_id,
            revision: 0,
            byte_length: length,
        };
        peer.transfers.insert(
            transfer_id,
            OutgoingBuffer {
                manifest: transfer.clone(),
                bytes: Arc::from(bytes),
                offset: 0,
                peer_acknowledged: false,
            },
        );
        self.downloads.insert(
            request_id,
            PendingDownload {
                peer: id,
                transfer_id,
            },
        );
        if let Err(error) = self.queue_message(
            id,
            ServerMessage::Effect {
                peer: scope,
                effect: ClientEffect::DownloadFile {
                    request_id,
                    file_name,
                    mime_type,
                    transfer,
                },
            },
        ) {
            self.downloads.remove(&request_id);
            if let Some(peer) = self.peers.get_mut(&id) {
                peer.transfers.remove(&transfer_id);
            }
            self.finish_download_storage(id, request_id, Some(error.to_string()));
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn download_completed(
        &mut self,
        id: ApplicationPeerId,
        request_id: u32,
        failure: Option<String>,
    ) -> Result<(), ApplicationServerError> {
        let Some(pending) = self.downloads.get(&request_id) else {
            return Ok(());
        };
        if pending.peer != id {
            return Err(ApplicationServerError::Admission(
                "download completion belongs to another peer",
            ));
        }
        let pending = self
            .downloads
            .remove(&request_id)
            .expect("admitted download completion");
        if let Some(peer) = self.peers.get_mut(&id) {
            peer.transfers.remove(&pending.transfer_id);
        }
        self.finish_download_storage(id, request_id, failure);
        Ok(())
    }

    fn finish_download_storage(
        &mut self,
        peer: ApplicationPeerId,
        request_id: u32,
        failure: Option<String>,
    ) {
        self.authority
            .shutdown
            .observe(&ApplicationInput::DownloadCompleted {
                request_id,
                failure: failure.clone(),
            });
        let notification = self
            .authority
            .storage
            .downloaded(request_id, failure.clone());
        if let Some(cause) = failure {
            self.authority.diagnostic = Some(cause);
        }
        if let Some(notification) = notification
            && let Some(peer) = self.peers.get_mut(&peer)
            && let PeerState::Ready(display) = &mut peer.state
        {
            display.platform.notification = notification;
        }
    }

    pub(super) fn retire_input_operations(
        &mut self,
        id: ApplicationPeerId,
    ) -> Result<(), ApplicationServerError> {
        let pickers: Vec<_> = self
            .pickers
            .iter()
            .filter_map(|(request, picker)| (picker.peer == id).then_some(*request))
            .collect();
        for request in pickers {
            let picker = self.pickers.remove(&request).expect("retired picker");
            self.finish_configuration(
                picker.document,
                request,
                ApplicationConfigurationLoad::Cancelled,
            )?;
        }
        let uploads: Vec<_> = self
            .uploads
            .keys()
            .filter(|(peer, _)| *peer == id)
            .copied()
            .collect();
        for key in uploads {
            let upload = self.uploads.remove(&key).expect("retired upload");
            self.finish_configuration(
                upload.picker.document,
                upload.request_id,
                ApplicationConfigurationLoad::Cancelled,
            )?;
        }
        Ok(())
    }

    pub(super) fn retire_all_operations(
        &mut self,
        id: ApplicationPeerId,
    ) -> Result<(), ApplicationServerError> {
        self.retire_input_operations(id)?;
        let downloads: Vec<_> = self
            .downloads
            .iter()
            .filter_map(|(request, pending)| (pending.peer == id).then_some(*request))
            .collect();
        for request in downloads {
            self.download_completed(
                id,
                request,
                Some("Client connection ended before export submission completed".to_owned()),
            )?;
        }
        Ok(())
    }
}
