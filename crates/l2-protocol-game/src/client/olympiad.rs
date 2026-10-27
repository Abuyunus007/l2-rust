//! Client packets of the Olympiad: the match list, the observer mode and the hero words.

// Java: gameserver/network/clientpackets/RequestOlympiadMatchList.java
client_unit_packet!(
    /// The player asks for the list of running matches (`0xD0` 0x002E). Java reads nothing and
    /// only answers while the player watches a match.
    RequestOlympiadMatchList,
    ex RequestOlympiadMatchList
);

// Java: gameserver/network/clientpackets/RequestOlympiadObserverEnd.java
client_unit_packet!(
    /// The player leaves the Olympiad observer mode (`0xD0` 0x0029). Java reads nothing.
    RequestOlympiadObserverEnd,
    ex RequestOlympiadObserverEnd
);

// Java: gameserver/network/clientpackets/RequestExOlympiadMatchListRefresh.java
client_unit_packet!(
    /// The player refreshes the match list (`0xD0` 0x0088). Java reads nothing.
    RequestExOlympiadMatchListRefresh,
    ex RequestExOlympiadMatchListRefresh
);

// Java: gameserver/network/clientpackets/ObserverReturn.java
client_unit_packet!(
    /// The observer goes back to where it watched from (`0xC1`). Java reads nothing.
    ObserverReturn,
    plain ObserverReturn
);

// Java: gameserver/network/clientpackets/RequestWriteHeroWords.java
client_packet!(
    /// A hero writes the message of the hero page (`0xD0` 0x0005). Java rejects more than 300
    /// characters; that check is the world's, the packet keeps the text as it came.
    RequestWriteHeroWords,
    ex RequestWriteHeroWords {
        /// Java `_heroWords` (`readString()`).
        hero_words: String,
    }
);
