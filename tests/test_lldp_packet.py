"""Unit tests for LLDP packet encoding and decoding (winlldp/lldp_packet.py)"""
import ipaddress
import struct
import unittest

from winlldp.lldp_packet import (
    ChassisIdSubtype, LLDPPacket, PortIdSubtype, TLV, TLVType, format_management_address,
)


def sample_packet():
    packet = LLDPPacket()
    packet.add_chassis_id(ChassisIdSubtype.MAC_ADDRESS, bytes.fromhex('001122334455'))
    packet.add_port_id(PortIdSubtype.INTERFACE_NAME, b'Ethernet 2')
    packet.add_ttl(120)
    packet.add_port_description('Uplink')
    packet.add_system_name('pc-01')
    packet.add_system_description('Windows 11')
    packet.add_system_capabilities(0x0080, 0x0080)
    return packet


class TestTLV(unittest.TestCase):

    def test_header_is_7_bit_type_and_9_bit_length(self):
        encoded = TLV(TLVType.SYSTEM_NAME, b'x' * 300).encode()
        header = struct.unpack('!H', encoded[:2])[0]
        self.assertEqual(header >> 9, TLVType.SYSTEM_NAME)
        self.assertEqual(header & 0x1FF, 300)
        self.assertEqual(len(encoded), 302)

    def test_decode_round_trip(self):
        tlv, offset = TLV.decode(TLV(TLVType.PORT_DESCRIPTION, b'abc').encode())
        self.assertEqual((tlv.type, tlv.value, offset), (TLVType.PORT_DESCRIPTION, b'abc', 5))


class TestPacketRoundTrip(unittest.TestCase):

    def test_encode_decode_to_dict(self):
        packet = sample_packet()
        packet.add_end_of_lldpdu()
        info = LLDPPacket.decode(packet.encode()).to_dict()
        self.assertEqual(info['chassis_id'], '00:11:22:33:44:55')
        self.assertEqual(info['chassis_id_subtype'], 'MAC_ADDRESS')
        self.assertEqual(info['port_id'], 'Ethernet 2')
        self.assertEqual(info['ttl'], 120)
        self.assertEqual(info['port_description'], 'Uplink')
        self.assertEqual(info['system_name'], 'pc-01')
        self.assertEqual(info['system_description'], 'Windows 11')
        self.assertEqual((info['system_capabilities'], info['enabled_capabilities']), (0x80, 0x80))

    def test_decoding_stops_at_end_of_lldpdu(self):
        packet = sample_packet()
        packet.add_end_of_lldpdu()
        data = packet.encode() + TLV(TLVType.SYSTEM_NAME, b'garbage').encode()
        self.assertEqual(LLDPPacket.decode(data).to_dict()['system_name'], 'pc-01')


class TestManagementAddress(unittest.TestCase):
    """Issue #1: the neighbor's IP address comes from its Management Address TLV"""

    def decode(self, *addresses):
        packet = sample_packet()
        for family, raw in addresses:
            packet.add_management_address(family, raw, 3)
        packet.add_end_of_lldpdu()
        return LLDPPacket.decode(packet.encode()).to_dict()

    def test_ipv4(self):
        info = self.decode((1, ipaddress.IPv4Address('192.168.10.1').packed))
        self.assertEqual(info['management_address'], '192.168.10.1')
        self.assertEqual(info['management_addresses'], ['192.168.10.1'])

    def test_ipv6_in_standard_notation(self):
        info = self.decode((2, ipaddress.IPv6Address('fe80::1').packed))
        self.assertEqual(info['management_address'], 'fe80::1')

    def test_several_addresses_prefer_ipv4(self):
        info = self.decode((2, ipaddress.IPv6Address('2001:db8::10').packed),
                           (1, ipaddress.IPv4Address('10.0.0.2').packed))
        self.assertEqual(info['management_addresses'], ['2001:db8::10', '10.0.0.2'])
        self.assertEqual(info['management_address'], '10.0.0.2')

    def test_no_management_address(self):
        info = self.decode()
        self.assertNotIn('management_address', info)

    def test_other_address_families_and_malformed_values(self):
        mac = struct.pack('!BB', 7, 6) + bytes.fromhex('aabbccddeeff')  # family 6 = 802 MAC
        self.assertEqual(format_management_address(mac), 'aa:bb:cc:dd:ee:ff')
        self.assertIsNone(format_management_address(b'\x05'))


if __name__ == '__main__':
    unittest.main()
