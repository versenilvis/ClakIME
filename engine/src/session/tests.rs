    use super::*;

    #[test]
    fn test_session_lifecycle_and_typing() {
        let mut session = ClakSession::new(Method::Telex);
        assert_eq!(*session.state(), SessionState::Idle);

        // type 't'
        let list = session.process_key(
            't' as u32,
            "t",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);
        unsafe {
            assert_eq!((*list.commands).cmd_type, CMD_FORWARD_KEY);
            assert_eq!((*list.commands).sym, 't' as u32);
        }
        assert!(session.context().is_composing());

        // type 'i'
        let list = session.process_key(
            'i' as u32,
            "i",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);
        unsafe {
            assert_eq!((*list.commands).cmd_type, CMD_FORWARD_KEY);
        }

        // type 'e'
        let list = session.process_key(
            'e' as u32,
            "e",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 1);

        // type 'e' -> replace 'e' with 'ê'
        let list = session.process_key(
            'e' as u32,
            "e",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );
        assert_eq!(list.count, 2);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_DELETE_SURROUNDING);
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_COMMIT_STRING);
            let s = std::ffi::CStr::from_ptr((*list.commands.add(1)).text)
                .to_str()
                .unwrap();
            assert_eq!(s, "ê");
        }
    }

    #[test]
    fn test_session_uinput_buffering_and_sentinel_recovery() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput -> transform to 'ư' using uinput
        let list = session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert_eq!(list.count, 2);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_EMIT_UINPUT_BACKSPACE);
            assert_eq!((*list.commands.add(0)).count, 2); // 1 real + 1 sentinel
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_START_TIMER);
            assert_eq!((*list.commands.add(1)).timer_id, TIMER_SAFETY);
        }
        assert!(session.is_deleting());
        assert_eq!(session.expected_backspaces(), 2);
        assert_eq!(session.pending_commit(), "ư");

        // incoming key while deleting must be buffered
        let list2 = session.process_key(
            'a' as u32,
            "a",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1005,
        );
        assert_eq!(list2.count, 0);
        assert_eq!(session.buffered_keys_count(), 1);

        // sentinel received -> flushes pending commit and buffered keys
        let list3 = session.on_sentinel_received(1020);
        assert!(list3.count >= 2);
        assert_eq!(session.buffered_keys_count(), 0);
        assert!(!session.is_deleting());
    }

    #[test]
    fn test_session_buffer_overflow_finalizes_commit() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput
        session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert!(session.is_deleting());

        // buffer 32 keys
        for _ in 0..MAX_BUFFERED_KEYS {
            let l = session.process_key(
                'a' as u32,
                "a",
                false,
                None,
                0,
                0,
                DELETION_UINPUT,
                0,
                10,
                5,
                2,
                100,
                250_000,
                1005,
            );
            assert_eq!(l.count, 0);
        }
        assert_eq!(session.buffered_keys_count(), MAX_BUFFERED_KEYS);

        // 33rd key triggers overflow protection
        let overflow_list = session.process_key(
            'z' as u32,
            "z",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1010,
        );
        // commit 'ư' + 2 cancel timers + 32 'a' forwards + 1 'z' forward
        assert_eq!(overflow_list.count, 1 + 2 + MAX_BUFFERED_KEYS + 1);
        assert!(!session.is_deleting());
        assert_eq!(session.buffered_keys_count(), 0);
    }

    #[test]
    fn test_session_batch_absorption_during_replay() {
        let mut session = ClakSession::new(Method::Telex);

        // type 'u'
        session.process_key(
            'u' as u32,
            "u",
            false,
            None,
            0,
            0,
            DELETION_SURROUNDING,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        // type 'w' with uinput
        session.process_key(
            'w' as u32,
            "w",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1000,
        );
        assert!(session.is_deleting());

        // buffer 'o', 'n', 'g' while deleting
        session.process_key(
            'o' as u32,
            "o",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1005,
        );
        session.process_key(
            'n' as u32,
            "n",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1006,
        );
        session.process_key(
            'g' as u32,
            "g",
            false,
            None,
            0,
            0,
            DELETION_UINPUT,
            0,
            10,
            5,
            2,
            100,
            250_000,
            1007,
        );
        assert_eq!(session.buffered_keys_count(), 3);

        // sentinel received -> batched commit "ương"
        let list = session.on_sentinel_received(1020);
        assert_eq!(list.count, 3);
        unsafe {
            assert_eq!((*list.commands.add(0)).cmd_type, CMD_CANCEL_TIMER);
            assert_eq!((*list.commands.add(1)).cmd_type, CMD_CANCEL_TIMER);
            assert_eq!((*list.commands.add(2)).cmd_type, CMD_COMMIT_STRING);
            let s = std::ffi::CStr::from_ptr((*list.commands.add(2)).text)
                .to_str()
                .unwrap();
            assert_eq!(s, "ương");
        }
    }
